//! G-LINKTRACE — the finishing link stage must not delete the scallop's
//! semantic trace.
//!
//! # The defect
//!
//! Switching the stage on collapsed a contour scallop's semantic trace to
//! nothing — 25 items to 1, regions 1 to 0, rings 23 to 0 — while the
//! TOOLPATH stayed correct. Narration, the GUI span list and the MCP trace
//! all went blank for the op; only the motion survived.
//!
//! `ScallopAnnotationChannel` asked `MoveProvenance::remap_range(i, i + 1)`.
//! Under `Permutation` that applies the foreign-intrusion rule, and the
//! relinker produces intrusion at EVERY junction by construction: it maps
//! the deleted junction rapid onto the whole replacement junction, and the
//! next fragment's first move onto that junction's last output, so the two
//! ranges overlap. Every ring annotation was therefore dropped. `reorder`
//! alone does it; the fixture below rotates nothing (`rotated_loops == 0`).
//!
//! The fix is `remap_point`: a single-move ANCHOR has no width, so it
//! cannot widen, so the rule that guards a multi-move CLAIM does not apply.
//! Plus a stable sort into emitted order, because `annotate_scallop` reads
//! this vector in order to derive each ring's end and to group regions.
//!
//! # Two traps this file is shaped around
//!
//! `ScallopReport::ring_count` is computed AFTER the reconcile, so it reads
//! 0 on the broken binary and `rings == ring_count` passes VACUOUSLY as
//! `0 == 0`. The population is therefore read from `relink.fragments`,
//! which the stage publishes before any of this.
//!
//! And "the fix moved no motion" must be MEASURED, not asserted by
//! construction. The digest below was captured on the pre-fix binary and is
//! pinned as a literal.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::meshes::sawtooth_plate;
use common::session::{generate, mesh_model, single_op_session_with, stock_over};
use common::tools::ball_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::ScallopConfig;
use rs_cam_core::semantic_trace::ToolpathSemanticKind;
use rs_cam_core::session::ProjectSession;

const HALF_MM: f64 = 20.0;
const PERIOD_MM: f64 = 6.0;
const AMPLITUDE_MM: f64 = 2.0;
const BALL_DIAMETER_MM: f64 = 3.0;

fn corrugated_session(hookup_mm: f64) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(
            sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
            "corrugated",
        ),
        "Scallop",
        OperationConfig::Scallop(ScallopConfig {
            intra_pass_hookup_mm: hookup_mm,
            ..ScallopConfig::default()
        }),
        |_cfg| {},
    );
    generate(&mut session, 0);
    session
}

/// A stable digest of every emitted move target, in order. The fix touches
/// the annotation reconcile only, so this must not move.
fn move_digest(session: &ProjectSession) -> u64 {
    let tp = session.get_result(0).expect("generated").toolpath();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for m in &tp.moves {
        for v in [m.target.x, m.target.y, m.target.z] {
            for b in v.to_bits().to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

fn report(session: &ProjectSession, arm: &str) {
    let result = session.get_result(0).expect("generated");
    let items = result
        .semantic_trace
        .as_ref()
        .map(|t| t.items.as_slice())
        .unwrap_or(&[]);
    let rings = items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Ring)
        .count();
    let regions = items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
        .count();
    let linked = items.iter().filter(|i| i.move_start.is_some()).count();
    let totals = result.stats.relink.as_ref();
    println!(
        "G-LINKTRACE {arm}: items {} (move-linked {}) · regions {} · rings {} \
         · fragments {:?} · rotated {:?} · at_depth {:?} · digest {:016x}",
        items.len(),
        linked,
        regions,
        rings,
        totals.map(|t| t.fragments),
        totals.map(|t| t.rotated_loops),
        totals.map(|t| t.at_depth_links),
        move_digest(session),
    );
}

/// Captured on the PRE-FIX binary, 2026-09-10, with the trace collapsed.
/// The fix touches the annotation reconcile only, so this must not move.
/// If it does, the change reached emitted motion and the claim is false.
const STAGE_ON_MOVE_DIGEST: u64 = 0x4512_f8cc_b89d_9097;

#[test]
fn the_link_stage_keeps_the_scallop_trace_and_moves_no_motion() {
    let on = corrugated_session(3.0);
    report(&on, "stage ON ");

    let result = on.get_result(0).expect("generated");

    // Population FIRST, from upstream of the thing under test. `ring_count`
    // is computed after the reconcile and reads 0 on the broken binary, so
    // asserting against it would pass vacuously — see the module doc.
    let totals = result
        .stats
        .relink
        .as_ref()
        .expect("the stage must publish totals, or this fixture is not on it");
    assert!(
        totals.fragments > 1,
        "population: the stage saw {} fragment(s), so it took no junction          decision and nothing below is being tested: {totals:?}",
        totals.fragments
    );

    let items = result
        .semantic_trace
        .as_ref()
        .map(|t| t.items.as_slice())
        .unwrap_or(&[]);
    let rings = items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Ring)
        .count();
    let regions = items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
        .count();

    assert!(
        rings > 0,
        "G-LINKTRACE: the stage ran over {} fragment(s) and the trace          carries no Ring at all — the intrusion rule ate the annotations          again",
        totals.fragments
    );
    assert!(
        regions > 0,
        "G-LINKTRACE: no Region survived, which is the `regions 0`          narration reads on a collapsed trace"
    );
    assert!(
        items.iter().all(|i| i.move_start.is_some()),
        "G-LINKTRACE: an item survived the reconcile UNLINKED; a ring          anchor either maps somewhere or the ring is gone, never both"
    );

    assert_eq!(
        move_digest(&on),
        STAGE_ON_MOVE_DIGEST,
        "G-LINKTRACE: emitted motion CHANGED. This fix is a provenance fix          and must not move a toolpath byte; the digest was captured on the          pre-fix binary."
    );
}

/// The stage-OFF arm keeps its trace too — it always did, and this is the
/// control that says the ON assertion above is about the stage.
#[test]
fn the_legacy_path_keeps_its_trace() {
    let off = corrugated_session(0.0);
    report(&off, "stage OFF");
    let items = off
        .get_result(0)
        .expect("generated")
        .semantic_trace
        .as_ref()
        .map(|t| t.items.len())
        .unwrap_or(0);
    assert!(items > 1, "the legacy path lost its trace: {items} item(s)");
}
