//! Feeds contract P21a (`planning/feeds_contract_2026-11-03/phases/P21a.md`):
//! every cap on the `ConstantChip` RPM (P21b renamed `MaxSpeed`) is a named limit.
//!
//! The grid is every flat pocket and flat 2D adaptive cell of the
//! literature matrix (`tests/literature_matrix/cells.toml`): 1 to 12 mm,
//! the 3, 6, 10 and 12 mm tiers included. The machine is the cell's own
//! (the shim's `resolve_machine`). No grid value is typed here.
//!
//! On each cell, under `ConstantChip`:
//! - the calculator returns an RPM choice;
//! - the RPM of the choice is the minimum of its named limits;
//! - the binding limit is the limit with that minimum;
//! - the shipped RPM is the RPM of the choice, unless a later step
//!   (Step 6 power, Step 7 feed ceiling) lowered it and says so.
//!
//! Under `MatchChart` the choice is `None`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    dead_code,
    reason = "test code: a panic is how a test reports a failure; the shared matrix modules keep helpers that this test does not call"
)]

#[path = "literature_matrix/cell.rs"]
mod cell;
#[path = "literature_matrix/expr.rs"]
mod expr;
#[path = "literature_matrix/shim.rs"]
mod shim;

use cell::{CellsFile, LiteratureCell};
use rs_cam_core::feeds::{RpmLimit, SpindleScaleReason, SpindleStrategy};

/// The flat milling cells of the literature matrix.
fn grid() -> Vec<LiteratureCell> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/literature_matrix/cells.toml");
    let src = std::fs::read_to_string(&path).expect("read cells.toml");
    let parsed: CellsFile = toml::from_str(&src).expect("parse cells.toml");
    parsed
        .cells
        .into_iter()
        .filter(|c| {
            c.inputs.tool_class == "flat"
                && matches!(c.inputs.operation.as_str(), "pocket" | "adaptive2d")
        })
        .collect()
}

/// A later step lowered the RPM after the choice, and named itself.
fn lowered_after_the_choice(reason: SpindleScaleReason) -> bool {
    match reason {
        SpindleScaleReason::PowerLimit | SpindleScaleReason::FeedCeiling => true,
        SpindleScaleReason::Unchanged | SpindleScaleReason::MaxSpeedPolicy => false,
    }
}

#[test]
fn every_rpm_cap_is_a_named_limit_p21() {
    let cells = grid();
    let mut diameters: Vec<f64> = Vec::new();
    let mut bindings: Vec<RpmLimit> = Vec::new();
    for cell in &cells {
        // A cell with no Suggest basis refuses (the matrix grades that
        // refusal); it has no RPM to choose.
        let Ok(snap) = shim::run_cell(cell) else {
            continue;
        };
        diameters.push(cell.inputs.diameter_mm);
        let choice = snap
            .rpm_choice
            .as_ref()
            .unwrap_or_else(|| panic!("{}: ConstantChip gave no RPM choice", cell.id));
        let min = choice
            .limits
            .iter()
            .min_by(|a, b| a.rpm.total_cmp(&b.rpm))
            .unwrap_or_else(|| panic!("{}: the choice has no limit", cell.id));
        // Step 2b climbs as `rpm x (ceiling / rpm)`, which can differ from
        // the ceiling by a rounding step; 1e-9 of the RPM covers it.
        let tol = 1e-9 * choice.rpm.abs().max(1.0);
        assert!(
            (choice.rpm - min.rpm).abs() <= tol,
            "{}: the RPM of the choice {} is not the minimum of the named limits {:?}",
            cell.id,
            choice.rpm,
            choice.limits,
        );
        assert_eq!(
            choice.binding, min.limit,
            "{}: the binding limit is not the minimum of {:?}",
            cell.id, choice.limits,
        );
        if lowered_after_the_choice(snap.spindle_scale_reason) {
            assert!(
                snap.rpm < choice.rpm,
                "{}: {:?} named a lower RPM, but the shipped RPM {} is not below the choice {}",
                cell.id,
                snap.spindle_scale_reason,
                snap.rpm,
                choice.rpm,
            );
        } else {
            assert!(
                (snap.rpm - choice.rpm).abs() <= tol,
                "{}: the shipped RPM {} is not the RPM of the choice {} ({:?})",
                cell.id,
                snap.rpm,
                choice.rpm,
                snap.spindle_scale_reason,
            );
        }
        if !bindings.contains(&choice.binding) {
            bindings.push(choice.binding);
        }

        let chart = shim::run_cell_with_strategy(cell, SpindleStrategy::MatchChart)
            .unwrap_or_else(|e| panic!("{}: the shim refused under MatchChart: {e}", cell.id));
        assert!(
            chart.rpm_choice.is_none(),
            "{}: MatchChart gave an RPM choice {:?}",
            cell.id,
            chart.rpm_choice,
        );
    }
    diameters.sort_by(f64::total_cmp);
    diameters.dedup();
    // The cells that run hold the four tiers that the task names (3, 6,
    // 10 and 12 mm); a matrix edit that drops a size shows here.
    assert!(
        diameters.len() >= 4,
        "the flat milling cells that run hold too few sizes: {diameters:?}"
    );
    // Non-vacuity: the grid binds at the tier, at the speed-up and at a
    // printed vendor maximum (the headroom sits above every tier on this
    // machine, so it binds on no cell of the grid).
    for limit in [
        RpmLimit::LegacyTier,
        RpmLimit::LegacySpeedup,
        RpmLimit::VendorRpmMax,
    ] {
        assert!(
            bindings.contains(&limit),
            "no cell of the grid binds at {limit:?}: {bindings:?} over {diameters:?}"
        );
    }
}
