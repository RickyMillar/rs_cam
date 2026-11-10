//! Feeds contract P21 (`planning/feeds_contract_2026-11-03/phases/P21a.md`,
//! `P21c.md`): every cap on the `ConstantChip` RPM is a named limit.
//!
//! The grid is every flat pocket and flat 2D adaptive cell of the
//! literature matrix (`tests/literature_matrix/cells.toml`): 1 to 12 mm.
//! The machine is the cell's own preset with its axis rates and
//! accelerations stated (`shim::stated_rates_machine`: a preset alone
//! refuses `ConstantChip`). The operator ceiling is the spindle maximum of
//! that machine. No grid value is typed here.
//!
//! On each cell, under `ConstantChip` with the ceiling:
//! - the calculator returns an RPM choice;
//! - the operator ceiling, the spindle maximum and the vendor maximum
//!   always bind: a chart RPM above the lowest of them comes down to it;
//! - any other limit below the chart RPM only stops the rise: the RPM is
//!   the chart RPM and the binding limit is that limit;
//! - else the RPM is the lowest limit, as a whole RPM that the spindle
//!   runs; with no chart RPM it is the lowest limit;
//! - the shipped RPM is the RPM of the choice, unless a later step
//!   (Step 6 power, Step 7 feed ceiling) lowered it and says so;
//! - the chip per rev (feed over RPM) of the choice is the `MatchChart`
//!   chip per rev when no later step moved the feed.
//!
//! With an empty ceiling, a cell whose row prints an RPM ships the
//! `MatchChart` RPM and feed. Under `MatchChart` the choice is `None`.

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

/// The limits that lower a chart RPM above them (lead ruling 2026-11-10).
fn always_binds(limit: RpmLimit) -> bool {
    match limit {
        RpmLimit::OperatorCeiling | RpmLimit::SpindleMax | RpmLimit::VendorRpmMax => true,
        RpmLimit::CuttingCeiling(_)
        | RpmLimit::ZAxisRate
        | RpmLimit::PowerAtRpm
        | RpmLimit::ChartRpm => false,
    }
}

/// A later step lowered the RPM after the choice, and named itself.
fn lowered_after_the_choice(reason: SpindleScaleReason) -> bool {
    match reason {
        SpindleScaleReason::PowerLimit | SpindleScaleReason::FeedCeiling => true,
        SpindleScaleReason::Unchanged | SpindleScaleReason::ConstantChip => false,
    }
}

#[test]
fn every_rpm_cap_is_a_named_limit_p21() {
    let cells = grid();
    let mut diameters: Vec<f64> = Vec::new();
    let mut bindings: Vec<RpmLimit> = Vec::new();
    let mut rises = 0_usize;
    for cell in &cells {
        // A cell with no Suggest basis refuses (the matrix grades that
        // refusal); it has no RPM to choose.
        let Ok(chart) = shim::run_cell_with_strategy(cell, SpindleStrategy::MatchChart) else {
            continue;
        };
        assert!(
            chart.rpm_choice.is_none(),
            "{}: MatchChart gave an RPM choice {:?}",
            cell.id,
            chart.rpm_choice,
        );
        let machine = shim::stated_rates_machine(cell).expect("the machine of the cell");
        let (_, spindle_max) = machine.rpm_range();
        let ceiling = SpindleStrategy::ConstantChip {
            rpm_ceiling: Some(spindle_max as u32),
        };
        let snap = shim::run_cell_on(cell, ceiling, &machine)
            .unwrap_or_else(|e| panic!("{}: ConstantChip refused: {e}", cell.id));
        diameters.push(cell.inputs.diameter_mm);
        let choice = snap
            .rpm_choice
            .as_ref()
            .unwrap_or_else(|| panic!("{}: ConstantChip gave no RPM choice", cell.id));
        let lowest_of = |hard: Option<bool>| {
            choice
                .limits
                .iter()
                .filter(|l| hard.is_none_or(|want| always_binds(l.limit) == want))
                .min_by(|a, b| a.rpm.total_cmp(&b.rpm))
                .copied()
        };
        let min = lowest_of(None).unwrap_or_else(|| panic!("{}: the choice has no limit", cell.id));
        let hard =
            lowest_of(Some(true)).unwrap_or_else(|| panic!("{}: no always-binding limit", cell.id));
        let snap_down = |rpm: f64| machine.next_rpm_at_or_below(rpm.floor());
        // The rule: an always-binding limit below the chart RPM lowers the
        // RPM; any other limit below the chart RPM only stops the rise.
        let (want_binding, expected) = match choice.chart_rpm {
            Some(chart) if hard.rpm < chart => (hard.limit, snap_down(hard.rpm)),
            Some(chart) if min.rpm < chart => (min.limit, chart),
            Some(_) | None => (min.limit, snap_down(min.rpm)),
        };
        assert_eq!(
            choice.binding, want_binding,
            "{}: the binding limit over {:?}",
            cell.id, choice.limits,
        );
        assert_eq!(
            choice.rpm, expected,
            "{}: the RPM of the choice over chart {:?} and {:?}",
            cell.id, choice.chart_rpm, choice.limits,
        );
        if choice.chart_rpm.is_some_and(|c| choice.rpm > c) {
            rises += 1;
        }
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
            assert_eq!(
                snap.rpm, choice.rpm,
                "{}: the shipped RPM is not the RPM of the choice ({:?})",
                cell.id, snap.spindle_scale_reason,
            );
        }
        if !bindings.contains(&choice.binding) {
            bindings.push(choice.binding);
        }

        // An empty ceiling: a row that prints an RPM ships the chart
        // values. (A row that prints none refuses; the sentry
        // `constant_chip_refuses_without_machine_rates_p21` holds that.)
        let empty = SpindleStrategy::ConstantChip { rpm_ceiling: None };
        if let Ok(no_ceiling) = shim::run_cell_on(cell, empty, &machine)
            && let Some(c) = no_ceiling.rpm_choice.as_ref()
        {
            let on_machine = shim::run_cell_on(cell, SpindleStrategy::MatchChart, &machine)
                .unwrap_or_else(|e| panic!("{}: MatchChart refused: {e}", cell.id));
            assert_eq!(
                (no_ceiling.rpm, no_ceiling.feed_rate_mm_min),
                (on_machine.rpm, on_machine.feed_rate_mm_min),
                "{}: an empty ceiling moved the chart values ({:?})",
                cell.id,
                c,
            );
        }
    }
    diameters.sort_by(f64::total_cmp);
    diameters.dedup();
    // The cells that run hold 3, 6, 10 and 12 mm; a matrix edit that
    // drops a size shows here.
    assert!(
        diameters.len() >= 4,
        "the flat milling cells that run hold too few sizes: {diameters:?}"
    );
    // Non-vacuity: the rule rises on the grid, and more than one limit
    // binds over it.
    assert!(rises > 0, "no cell of the grid rises above its chart RPM");
    assert!(
        bindings.len() >= 2,
        "one limit binds on every cell of the grid: {bindings:?} over {diameters:?}"
    );
}
