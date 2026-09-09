//! G-LINKVISIBLE — every finishing family that runs a link stage publishes
//! its counters on the STATS channel, so they reach `narrate_toolpath` and
//! MCP.
//!
//! # The defect
//!
//! `ToolpathStats::relink` had exactly ONE writer: the unified-finish arm of
//! `compute/execute.rs`. Every other family that G-LINKSTAGE (`0c36a2f0`)
//! put on the shared stage built a report and only LOGGED it — the scallop's
//! `"Scallop intra-pass relink"` line, the adapter's `"Finishing link
//! stage"` line for the raster and the waterline, and the pencil's `"Pencil
//! link stage"` line. The report was then dropped.
//!
//! So `at_depth_links` — the ACCEPTANCE measure for the whole stage, the one
//! counter that says an ENTRY was removed rather than merely a retract —
//! reached no operator surface at all on four of the five families. It was
//! not on `ToolpathStats`, so it was not in narration and not on MCP. The
//! only way anyone had ever read it was scraping a headless CLI's stdout. A
//! stage judged on counters nobody can see is not measurable.
//!
//! # What each arm asserts
//!
//! One arm per family, each generating through a real `ProjectSession` — the
//! same entry point the GUI worker and the CLI use, so the arm crosses
//! `compute/execute.rs` and fails if the adapter stops handing the stage in.
//!
//! Every arm asserts its POPULATION before its verdict, for the reason
//! `CLAUDE.md` gives: a gate handed an empty population passes and looks
//! healthy. If a fixture stops producing junctions, the arm must fail loudly
//! rather than pass vacuously on a report full of zeros.
//!
//! The SCALLOP arm is not here. It lives at the end of
//! `scallop_intra_pass_relink_am7.rs`
//! (`the_scallop_adapter_hands_the_link_stage_in`), next to the corrugated
//! session fixture that file already owns; splitting it out would duplicate
//! the fixture rather than the assertion. Four arms, two files.
//!
//! # Why the pencil has its own slot
//!
//! `PencilLinkReport` carries eight counters against `RelinkTotals`' six,
//! and the two a mapping would drop — `hop_too_far` and this pass's own
//! at-depth/hop split — are exactly the ones that name the pencil's binding
//! constraint. This repo has a long history of the opposite choice: the
//! chipload gate's units, the two air-cut denominators, the reach map's area
//! basis. A measurement squeezed into another measurement's shape reads
//! clean and means something else.
//!
//! # No behaviour change
//!
//! Nothing here moves a toolpath byte. The counters were already computed on
//! every one of these paths; only their destination changed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::meshes::sawtooth_plate;
use common::session::{generate, mesh_model, pinned_heights, single_op_session_with, stock_over};
use common::tools::ball_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{DropCutterConfig, PencilConfig, WaterlineConfig};
use rs_cam_core::pencil::PencilLinkReport;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::unified_finish::RelinkTotals;

/// The corrugated plate `scallop_intra_pass_relink_am7` uses: a triangular
/// wave with sharp crests and troughs, Y-invariant. Every family here wants
/// the same thing from it — many short cut runs whose ends nearly touch,
/// which is the topology a link stage exists for.
const HALF_MM: f64 = 20.0;
const PERIOD_MM: f64 = 6.0;
const AMPLITUDE_MM: f64 = 2.0;
/// Ø3 ball, as in A/M7 — a little under one period, so the ball FOLLOWS the
/// corrugation instead of bridging it (`sawtooth_plate`'s own caveat).
const BALL_DIAMETER_MM: f64 = 3.0;

/// Read the shared link-stage totals a generated toolpath published.
///
/// `expect` rather than a soft `None` arm: on these fixtures the stage is
/// switched ON, so `None` means the adapter never handed it in — the exact
/// defect this file exists for — and `None` is "NOT MEASURED", never
/// "nothing to link".
fn relink_totals(session: &ProjectSession, family: &str) -> RelinkTotals {
    session
        .get_result(0)
        .unwrap_or_else(|| panic!("{family}: generation produced no result"))
        .stats
        .relink
        .unwrap_or_else(|| {
            panic!(
                "G-LINKVISIBLE: {family} published no link-stage totals, so \
                 the stage never ran through compute/execute.rs. `None` here \
                 means NOT MEASURED, never 'nothing to link'."
            )
        })
}

/// Population, then verdict — in that order, in every arm.
///
/// The population check is `fragments > 1`: with one fragment there is no
/// junction, so every counter below it is vacuous and the fixture has
/// stopped testing the wiring. The verdict is `at_depth_links` OR
/// `clearance_hops`, because a family that declares no `FragmentKind` (the
/// raster and the waterline both) reports `rotated_loops == 0` by
/// construction and rotation cannot stand in for a link there.
fn assert_stage_did_something(totals: &RelinkTotals, family: &str) {
    assert!(
        totals.fragments > 1,
        "G-LINKVISIBLE population: {family} emitted {} fragment(s), so the \
         stage had no junction to act on and every assertion below it is \
         vacuous. Fix the fixture, do not relax the gate: {totals:?}",
        totals.fragments
    );
    assert!(
        totals.at_depth_links > 0 || totals.clearance_hops > 0,
        "G-LINKVISIBLE: {family} published totals but joined nothing — the \
         stage ran and declined every junction, which makes this arm blind \
         to whether the adapter hands the stage in at all: {totals:?}"
    );
    println!(
        "G-LINKVISIBLE {family}: fragments {} · at_depth {} · hops {} · \
         rotated {} · retracts {} · declined {}",
        totals.fragments,
        totals.at_depth_links,
        totals.clearance_hops,
        totals.rotated_loops,
        totals.retract_links,
        totals.declined()
    );
}

// ── Arm 1: drop_cutter ──────────────────────────────────────────────────

/// The raster's rows are split by the SLOPE filter: only the near-flat cells
/// around each crest and trough survive, so one row becomes many short runs
/// two or more grid steps apart — beyond the serpentine hookup's
/// one-diagonal cap, which is the gap G-LINKSTAGE put this family on the
/// shared stage for.
fn drop_cutter_session(hookup_mm: f64) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(
            sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
            "corrugated",
        ),
        "Drop Cutter",
        OperationConfig::DropCutter(DropCutterConfig {
            stepover: 0.5,
            slope_from: 0.0,
            slope_to: 20.0,
            hookup_mm,
            ..DropCutterConfig::default()
        }),
        |cfg| cfg.heights = pinned_heights(2.0, -1.0),
    );
    generate(&mut session, 0);
    session
}

#[test]
fn the_drop_cutter_adapter_publishes_its_link_counters() {
    let session = drop_cutter_session(3.0);
    let totals = relink_totals(&session, "drop_cutter");
    assert_stage_did_something(&totals, "drop_cutter");
}

/// The other half of the contract: with the dial at its SHIPPED `0.0` the
/// stage does not run, and the channel says **not measured** rather than
/// reporting a clean zero. Recording `Some(default)` here would be
/// indistinguishable from a stage that ran and found nothing.
#[test]
fn a_drop_cutter_with_the_stage_off_reads_not_measured() {
    let session = drop_cutter_session(0.0);
    let stats = &session.get_result(0).expect("baseline generated").stats;
    assert_eq!(
        stats.relink, None,
        "G-LINKVISIBLE: `hookup_mm` ships at 0.0, so no stage ran and the \
         slot must read NOT MEASURED. A zeroed report here would claim \
         every junction was considered and none declined."
    );
    assert_eq!(
        DropCutterConfig::default().hookup_mm,
        0.0,
        "the shipped dial is what makes the arm above a real off-state"
    );
}

// ── Arm 2: waterline ────────────────────────────────────────────────────

/// A waterline level on a Y-invariant corrugation is one contour per flank,
/// so a single Z level already emits many fragments whose ends sit within a
/// few millimetres of each other.
fn waterline_session(hookup_mm: f64) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(
            sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
            "corrugated",
        ),
        "Waterline",
        OperationConfig::Waterline(WaterlineConfig {
            z_step: 0.5,
            sampling: 0.3,
            hookup_mm,
            ..WaterlineConfig::default()
        }),
        // A surface op carries no depth dial, so `bottom_z: Auto` would
        // collapse the band to zero height and emit no move at all.
        |cfg| cfg.heights = pinned_heights(2.0, -2.0),
    );
    generate(&mut session, 0);
    session
}

#[test]
fn the_waterline_adapter_publishes_its_link_counters() {
    let session = waterline_session(3.0);
    let totals = relink_totals(&session, "waterline");
    assert_stage_did_something(&totals, "waterline");
}

// ── Arm 3: pencil ───────────────────────────────────────────────────────

/// The corrugation's troughs are the pencil's valleys — one per period, so
/// the detector emits several runs and the emitter has real junctions to
/// decide about.
fn pencil_session() -> ProjectSession {
    let mut session = single_op_session_with(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(
            sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
            "corrugated",
        ),
        "Pencil",
        OperationConfig::Pencil(PencilConfig {
            // One period plus slack: adjacent troughs are `PERIOD_MM` apart,
            // and the run-to-run gap the emitter sees is that distance.
            hookup_distance: PERIOD_MM * 1.5,
            ..PencilConfig::default()
        }),
        |cfg| cfg.heights = pinned_heights(2.0, -2.0),
    );
    generate(&mut session, 0);
    session
}

#[test]
fn the_pencil_adapter_publishes_its_own_link_counters() {
    let session = pencil_session();
    let report: PencilLinkReport = session
        .get_result(0)
        .expect("pencil generated")
        .stats
        .pencil_link
        .expect(
            "G-LINKVISIBLE: the pencil published no link report, so the \
             adapter never carried it out of the generator. `None` here \
             means NOT MEASURED — no centreline was emitted at all — never \
             'nothing to link'.",
        );

    // Population before verdict. `junctions` is every transition between two
    // emitted runs; at zero there was no link decision to take and every
    // counter below is vacuous.
    assert!(
        report.junctions > 0,
        "G-LINKVISIBLE population: the pencil emitted fewer than two runs, \
         so the stage had no junction to act on: {report:?}"
    );
    assert!(
        report.linked_at_depth > 0 || report.linked_via_hop > 0,
        "G-LINKVISIBLE: the pencil published a report but joined nothing, \
         which makes this arm blind to whether the adapter carries the \
         report at all: {report:?}"
    );

    // The slot's whole justification: the two counters a mapping onto
    // `RelinkTotals` would have to drop are PRESENT and readable here —
    // `hop_too_far` and `ceiling_refused` have no counterpart in the shared
    // shape, and the at-depth/hop split is what tells an entry-bound pass
    // from a retract-bound one.
    println!(
        "G-LINKVISIBLE pencil: junctions {} · at_depth {} · hops {} · \
         too_far {} · hop_too_far {} · off_surface {} · ceiling_refused {} \
         · slower_than_retract {}",
        report.junctions,
        report.linked_at_depth,
        report.linked_via_hop,
        report.too_far,
        report.hop_too_far,
        report.off_surface,
        report.ceiling_refused,
        report.slower_than_retract
    );
}

/// The pencil writes its OWN slot and never the shared one — the design
/// ruling, pinned so a later "tidy-up" that folds the two together fails
/// here rather than silently losing `hop_too_far`.
#[test]
fn the_pencil_never_writes_the_shared_relink_slot() {
    let session = pencil_session();
    let stats = &session.get_result(0).expect("pencil generated").stats;
    assert!(
        stats.pencil_link.is_some(),
        "the fixture must reach the pencil emitter for this arm to mean \
         anything"
    );
    assert_eq!(
        stats.relink, None,
        "G-LINKVISIBLE: the pencil runs a different linker with a different \
         counter set. Mapping it onto `RelinkTotals` would drop \
         `hop_too_far` and `ceiling_refused`, which are the counters that \
         name this pass's binding constraint."
    );
}

// ── The narration surface ───────────────────────────────────────────────

/// The counters reach `narrate_toolpath`, which is the surface an agent (and
/// MCP, which serves the same string) actually reads.
///
/// Asserted on the shared line and on the pencil line separately, because
/// they are two different reports and the narration prints them as two
/// lines with their own counter names.
#[test]
fn the_counters_reach_the_narration() {
    let session = drop_cutter_session(3.0);
    let narration = session
        .narrate_toolpath(0)
        .expect("narration for a generated toolpath");
    println!("--- drop_cutter narration ---\n{narration}");
    assert!(
        narration.contains("Link stage:"),
        "G-LINKVISIBLE: the shared link-stage line must reach narration, \
         which is what MCP's narrate_toolpath serves"
    );
    assert!(
        narration.contains("AT CUTTING DEPTH"),
        "an agent must be able to tell an at-depth link from a clearance \
         hop: only the first removes an entry"
    );
    assert!(
        narration.contains("clearance hop"),
        "and the hop tier must be named on the same line"
    );
    assert!(
        narration.contains("`hookup_mm`"),
        "the tuning lever named must be THIS family's dial — naming \
         `intra_region_hookup_mm` on a raster sends the reader to a field \
         the operation does not have"
    );

    let pencil = pencil_session();
    let pencil_narration = pencil
        .narrate_toolpath(0)
        .expect("narration for a generated pencil");
    println!("--- pencil narration ---\n{pencil_narration}");
    assert!(
        pencil_narration.contains("Pencil link stage:"),
        "the pencil's report is narrated on its own line, under its own \
         counter names"
    );
    assert!(
        pencil_narration.contains("hop_too_far"),
        "including the counter a mapping onto `RelinkTotals` would drop"
    );
}
