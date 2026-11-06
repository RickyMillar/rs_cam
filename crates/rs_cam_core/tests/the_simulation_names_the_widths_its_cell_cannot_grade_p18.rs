//! Feeds contract P18 item 4b (lead rulings, 2026-11-07): the simulation
//! names the widths its cell cannot grade.
//!
//! Source: `planning/feeds_review_2026-11-06/RECHECK_bd72997c9.md` D15. On
//! cell `P_thin_005` (a 6 mm flat at a 0.3 mm stepover) the project
//! simulates at 0.5 mm: the cell is wider than the stepover, the gates read
//! no engagement on real cuts, and the project verdict printed
//! "WARNING: 71.3% air cutting of total runtime" while the air-cut metric
//! was withheld.
//!
//! The fixture is that project (`fixtures/p_thin_005_2026-11-07_3fc31cc8`).
//! The arms:
//! - (a) the a-priori finding `project.sim_cell_above_half_stepover` fires
//!   at a 0.5 mm cell (0.5 > 0.3 / 2) and names the cell, the stepover and
//!   the cell to use (0.15 mm); at 0.1 mm it does not fire. It needs no
//!   simulation.
//! - (b) after a simulation at 0.5 mm, the abstention's fix hint names the
//!   stepover rule and 0.15 mm, the smaller of the two rules; the
//!   tip-contact rule reads the cusp radius (D/2 = 3 mm on the 6 mm flat),
//!   and the text calls it that, not a tip radius.
//! - (c) the project headline states no air-cut percentage that a
//!   measurability abstention withholds.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::session::{
    Command, ProjectSession, ProjectVerdict, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution, VerdictKind,
};

/// The stepover of the fixture's Pocket (mm).
const STEPOVER_MM: f64 = 0.3;
/// The cell the fixture stores (mm).
const STORED_CELL_MM: f64 = 0.5;
/// A cell at or below half the stepover (mm): 0.1 < 0.3 / 2.
const FINE_CELL_MM: f64 = 0.1;

fn load() -> ProjectSession {
    let path = common::repo_root()
        .join("crates/rs_cam_core/tests/fixtures/p_thin_005_2026-11-07_3fc31cc8/project.toml");
    ProjectSession::load(common::require_fixture(&path)).expect("the fixture loads")
}

fn set_cell(s: &mut ProjectSession, cell_mm: f64) {
    let _ = s
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(cell_mm),
            },
        ))
        .expect("the cell applies");
}

fn cell_findings(s: &ProjectSession) -> Vec<String> {
    s.diagnostics()
        .verdicts
        .iter()
        .filter(|v| v.kind == VerdictKind::SimCellAboveHalfStepover)
        .map(|v| format!("{} | {}", v.headline, v.fix_hint))
        .collect()
}

/// (a) The a-priori finding: before any simulation.
#[test]
fn a_cell_above_half_the_stepover_is_named_before_the_simulation_p18() {
    let mut s = load();
    assert_eq!(s.simulation_resolution_mm(), STORED_CELL_MM);
    let at_stored = cell_findings(&s);
    assert_eq!(at_stored.len(), 1, "{at_stored:?}");
    let text = at_stored.first().expect("one finding");
    let half = STEPOVER_MM / 2.0;
    for needle in [
        format!("cell {STORED_CELL_MM} mm"),
        format!("stepover {STEPOVER_MM} mm"),
        "2 cells".to_owned(),
        format!("{half} mm or finer"),
    ] {
        assert!(text.contains(&needle), "{needle:?} not in {text}");
    }
    set_cell(&mut s, FINE_CELL_MM);
    assert!(cell_findings(&s).is_empty(), "{:?}", cell_findings(&s));
}

/// Generate the Pocket and simulate it at the stored cell.
fn simulated() -> ProjectSession {
    let mut s = load();
    let cancel = AtomicBool::new(false);
    let _ = s
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates");
    let _ = s
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(STORED_CELL_MM),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the simulation runs");
    s
}

/// (b) and (c): the hint names the stepover cell; the headline states no
/// withheld percentage.
#[test]
fn the_abstention_names_the_cell_and_the_headline_withholds_the_percent_p18() {
    let s = simulated();
    let diag = s.diagnostics();
    let abstained: Vec<_> = diag
        .verdicts
        .iter()
        .filter(|v| v.kind == VerdictKind::MeasurabilityAbstained)
        .collect();
    assert!(
        !abstained.is_empty(),
        "non-vacuity: the 0.5 mm cell withholds the air cut: {:?}",
        diag.verdicts
            .iter()
            .map(|v| &v.headline)
            .collect::<Vec<_>>()
    );
    let hint = &abstained.first().expect("one abstention").fix_hint;
    let half = STEPOVER_MM / 2.0;
    assert!(
        hint.contains(&format!("{half} mm or finer")),
        "the hint names the stepover cell: {hint}"
    );
    // Lead ruling: a flat end mill has no tip radius. The rule reads the
    // cutter's cusp radius (`MillingCutter::cusp_radius`, D/2 = 3 mm here),
    // and the text names that quantity.
    let cusp_mm = s.tools().first().expect("the fixture's tool").diameter / 2.0;
    assert!(
        hint.contains(&format!(
            "tip-contact rule (below the cusp radius, {cusp_mm} mm)"
        )),
        "the hint names the cusp radius: {hint}"
    );
    assert!(!hint.contains("tip radius"), "{hint}");
    // (c) The aggregate air cut is above the bar, so the old headline
    // printed a percentage; the abstention withholds it now.
    let verdict = diag.verdict();
    let headline = verdict.headline();
    assert!(
        !matches!(verdict, ProjectVerdict::HighAirCut { .. }),
        "the air cut is withheld: {headline}"
    );
    assert!(!headline.contains('%'), "{headline}");
}
