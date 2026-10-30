//! G-SERIES (A) — the feed, the G6 ramp and the gate read one chart cell
//! (operator ruling 2026-10-31: "plunge, ramp, feed and the sim gate read
//! ONE row").
//!
//! The defect it guards: for a generic 6 mm 2-flute end mill in softwood,
//! Pocket roughing, the feed read the Amana ZrN carving row
//! (`amana-zrn-flat-softwood-pocket-6000-2f`, chip 0.2032) and the G6
//! ramp read the Amana Spektra row (`amana-flat-softwood-pocket-6000-2f-spektra`).
//! One operation took its feed and its ramp from two charts.
//!
//! Over every cell of the series walk (`common::feeds_cells`) that Suggest
//! answers, the sentry reads three rows:
//!
//! - the feed row: `FeedsResult::matched_lut_row`, the row that sets the
//!   chip, the RPM and the feed;
//! - the ramp row: the side row of the G6 drill claim
//!   (`RampBasis::Sourced`), where the cell has one and the feed row has the
//!   drill rule's home, (Pocket, Roughing), directly or through a G3 family
//!   transfer (lead ruling 2026-10-31);
//! - the gate row: `tool_load::chip_envelope_row` at the shipped depth, the
//!   row that the sim gate, the modulator and the advisor read.
//!
//! "One chart cell" is the printed cell: source, subfamily, material
//! family, diameter and flute count. The id is not the key, because the G6
//! drill rule reads the (Pocket, Roughing) side row for every family: an
//! Adaptive cell reads the Spektra adaptive row for its feed and the
//! Spektra pocket row of the same printed cell for its ramp.
//!
//! The G10 plunge reads no row. It is the family fraction of the shipped
//! feed (`plunge::resolve_plunge`), so it follows the feed row.
//!
//! A feed row that prints no chipload is not compared with the gate row:
//! the gate reads only chipload rows (Checkpoint K).

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::HashMap;

use rs_cam_core::feeds::EMBEDDED_LUT;
use rs_cam_core::feeds::ramp::RampBasis;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole, VendorObservation};

/// The printed chart cell of a row.
fn chart_cell(obs: &VendorObservation) -> String {
    format!(
        "{} / {} / {:?} / {:?} mm / {}F",
        obs.source_id,
        obs.tool_subfamily.as_deref().unwrap_or("-"),
        obs.material_family,
        obs.diameter_mm,
        obs.flute_count
    )
}

fn has_chipload(obs: &VendorObservation) -> bool {
    obs.chipload_min_mm_tooth.is_some() || obs.chipload_max_mm_tooth.is_some()
}

#[test]
fn the_feed_the_ramp_and_the_gate_read_one_chart_cell_g_series() {
    let by_id: HashMap<&str, &VendorObservation> = EMBEDDED_LUT
        .observations
        .iter()
        .map(|o| (o.observation_id.as_str(), o))
        .collect();
    let row = |id: &str| -> &VendorObservation {
        by_id
            .get(id)
            .copied()
            .unwrap_or_else(|| panic!("row {id} is in the embedded LUT"))
    };

    let mut failures = Vec::new();
    let mut ramp_checked = 0_usize;
    let mut gate_checked = 0_usize;
    for cell in common::feeds_cells::walk() {
        let Some(s) = cell.suggested.as_ref() else {
            continue;
        };
        let r = &s.feeds_result;
        let Some(feed) = r.matched_lut_row.as_ref() else {
            continue;
        };
        if feed.size_basis.is_refused() {
            continue;
        }
        let feed_obs = row(&feed.observation_id);
        let feed_cell = chart_cell(feed_obs);

        // The G6 ramp reads the (Pocket, Roughing) side row. The comparison
        // runs where the feed row has that home too, directly or through a
        // G3 family transfer (lead ruling 2026-10-31). A feed from a
        // (Contour, Finish) chart with a Spektra ramp is the open two-chart
        // class (`planning/toolpath_panel_review_2026-10-30/FEEDS_SPECIALTY_ROW_FIX.md`).
        let feed_home = feed
            .family_basis
            .claim()
            .map_or((feed_obs.operation_family, feed_obs.pass_role), |c| c.home);
        let pocket_roughing_home = feed_home == (LutOperationFamily::Pocket, LutPassRole::Roughing);
        if let RampBasis::Sourced { drill, .. } = &r.ramp
            && pocket_roughing_home
        {
            ramp_checked += 1;
            let ramp_cell = chart_cell(row(&drill.side_row));
            if ramp_cell != feed_cell {
                failures.push(format!(
                    "{}: the feed reads {} ({feed_cell}); the G6 ramp reads {} ({ramp_cell})",
                    cell.label(),
                    feed.observation_id,
                    drill.side_row
                ));
            }
        }

        if has_chipload(feed_obs) {
            let depth = s.operation.depth_per_pass().unwrap_or(r.axial_depth_mm);
            let gate = rs_cam_core::tool_load::chip_envelope_row(
                &cell.material,
                &cell.tool,
                &s.operation,
                depth,
            );
            gate_checked += 1;
            match gate {
                Some(g) if g.observation_id == feed.observation_id => {}
                Some(g) => failures.push(format!(
                    "{}: the feed reads {}; the gate reads {}",
                    cell.label(),
                    feed.observation_id,
                    g.observation_id
                )),
                None => failures.push(format!(
                    "{}: the feed reads {}; the gate reads no row",
                    cell.label(),
                    feed.observation_id
                )),
            }
        }
    }

    // Non-vacuity: the walk reaches both comparisons.
    assert!(ramp_checked > 0, "no cell carries a G6 ramp claim");
    assert!(gate_checked > 0, "no cell reaches the gate comparison");
    assert!(
        failures.is_empty(),
        "{} cells read more than one chart cell ({ramp_checked} ramp and {gate_checked} gate \
         comparisons):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Instrument: the open two-chart class. The cells where the G6 ramp reads
/// the Spektra (Pocket, Roughing) row and the feed reads a row of another
/// home (a finish chart). The lead ruled them open on 2026-10-31; the list
/// is in `planning/toolpath_panel_review_2026-10-30/FEEDS_SPECIALTY_ROW_FIX.md`.
#[test]
#[ignore = "instrument: prints the open two-chart cells; run by name with -- --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "the instrument prints its list for the reader"
)]
fn list_the_open_two_chart_cells_g_series() {
    let by_id: HashMap<&str, &VendorObservation> = EMBEDDED_LUT
        .observations
        .iter()
        .map(|o| (o.observation_id.as_str(), o))
        .collect();
    let mut count = 0_usize;
    for cell in common::feeds_cells::walk() {
        let Some(s) = cell.suggested.as_ref() else {
            continue;
        };
        let r = &s.feeds_result;
        let (Some(feed), RampBasis::Sourced { drill, .. }) = (r.matched_lut_row.as_ref(), &r.ramp)
        else {
            continue;
        };
        let (Some(f), Some(d)) = (
            by_id.get(feed.observation_id.as_str()),
            by_id.get(drill.side_row.as_str()),
        ) else {
            continue;
        };
        if chart_cell(f) != chart_cell(d) {
            count += 1;
            println!(
                "{} | feed {} ({:?}, {:?}) | ramp {}",
                cell.label(),
                feed.observation_id,
                f.operation_family,
                f.pass_role,
                drill.side_row
            );
        }
    }
    println!("open two-chart cells: {count}");
}
