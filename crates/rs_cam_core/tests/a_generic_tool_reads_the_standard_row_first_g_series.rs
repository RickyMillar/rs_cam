//! G-SERIES (B) — a generic tool reads the standard row first (operator
//! ruling 2026-10-31).
//!
//! The ruling: "A specialty-series row (ZrN carving, etc.) serves only a
//! tool of that series; a generic tool uses standard rows first and falls
//! back to a specialty row only when no standard row exists." The lead
//! refined it the same day: the series is a tie-breaker. Where two rows tie
//! on material and size, a generic tool reads the standard one; the score
//! decides every other choice (`vendor_lookup::prefer_series`).
//!
//! `feeds::series::SERIES_RULES` and the subfamily words in
//! `feeds::series::row_series` are the one classification. The arms:
//!
//! - the walk: no cell of the series walk (`common::feeds_cells`, a tool
//!   with no series) reads a specialty row while a standard row printed for
//!   the same material family at the same size serves the cell;
//! - the defect cell: the 6 mm 2-flute end mill, softwood, Pocket reads the
//!   Amana Spektra row, and ships its printed chip at the shipped RPM and
//!   feed (before the fix it read the ZrN carving row and shipped 0.2032);
//! - a tool that declares a series (the cut direction: downcut or
//!   compression) reads a row of its own series wherever one ties on
//!   material and size with the row it reads; a lookup query that names
//!   the ZrN carving series reads the ZrN row;
//! - the fallback: a cell that only a specialty row serves still ships it.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::BitCutDirection;
use rs_cam_core::compute::tool_config::ToolType;
use rs_cam_core::feeds::series::{ToolSeries, is_standard, row_series};
use rs_cam_core::feeds::suggest::feeds_input_for_operation;
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, find_best_row_for_geometry};
use rs_cam_core::feeds::vendor_lut::{
    LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut, VendorObservation,
};
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{EMBEDDED_LUT, SpindleStrategy};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const SPEKTRA_6MM_SOFTWOOD_POCKET: &str = "amana-flat-softwood-pocket-6000-2f-spektra";
const ZRN_6MM_SOFTWOOD_POCKET: &str = "amana-zrn-flat-softwood-pocket-6000-2f";

fn obs(id: &str) -> &'static VendorObservation {
    EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .unwrap_or_else(|| panic!("row {id} is in the embedded LUT"))
}

/// The embedded LUT with only the standard rows.
fn standard_lut() -> VendorLut {
    lut_where(is_standard)
}

/// The embedded LUT with only the rows that `keep` admits.
fn lut_where(keep: impl Fn(&VendorObservation) -> bool) -> VendorLut {
    VendorLut {
        observations: EMBEDDED_LUT
            .observations
            .iter()
            .filter(|o| keep(o))
            .cloned()
            .collect(),
    }
}

/// True when `o` is printed for the material family of `row` at its size:
/// the tie that the series decides.
fn ties_on_material_and_size(o: &VendorObservation, row: &VendorObservation) -> bool {
    o.material_family == row.material_family
        && match (o.diameter_mm, row.diameter_mm) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            (None, None) => true,
            _ => false,
        }
}

#[test]
fn no_generic_cell_reads_a_specialty_row_while_a_tied_standard_row_serves_g_series() {
    let machine = MachineProfile::default();
    let mut checked = 0_usize;
    let mut specialty_cells = 0_usize;
    let mut failures = Vec::new();
    for cell in common::feeds_cells::walk() {
        let Some(s) = cell.suggested.as_ref() else {
            continue;
        };
        let Some(feed) = s.feeds_result.matched_lut_row.as_ref() else {
            continue;
        };
        checked += 1;
        let feed_obs = obs(&feed.observation_id);
        if is_standard(feed_obs) {
            continue;
        }
        specialty_cells += 1;
        let tied = lut_where(|o| is_standard(o) && ties_on_material_and_size(o, feed_obs));
        let input = feeds_input_for_operation(
            &cell.operation,
            &cell.tool,
            &cell.material,
            &machine,
            &EMBEDDED_LUT,
            SpindleStrategy::default(),
        );
        let query = to_lookup_query(&input).expect("Suggest answered, so the routing answers");
        if let Some(std_row) = find_best_row_for_geometry(&tied, &query, &input.tool_geometry)
            && !std_row.size_basis.is_refused()
        {
            failures.push(format!(
                "{}: reads the specialty row {} while the standard row {} ties on material \
                 and size and serves the cell",
                cell.label(),
                feed.observation_id,
                std_row.observation_id
            ));
        }
    }
    assert!(checked > 0, "no walked cell reads a vendor row");
    assert!(
        failures.is_empty(),
        "{} of {checked} cells ({specialty_cells} on a specialty row) break the series \
         ruling:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_generic_6mm_softwood_pocket_ships_the_spektra_chip_g_series() {
    let cell = common::feeds_cells::walk()
        .into_iter()
        .find(|c| {
            c.kind == ToolType::EndMill
                && (c.diameter_mm - 6.0).abs() < 1e-9
                && c.flutes == 2
                && c.op == OperationType::Pocket
                && c.material_label == "softwood"
        })
        .expect("the walk holds the cell");
    let s = cell.suggested.as_ref().expect("Suggest answers the cell");
    let row = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("the cell reads a vendor row");
    assert_eq!(row.observation_id, SPEKTRA_6MM_SOFTWOOD_POCKET);
    // The printed point of the Spektra row (the chart prints one value).
    let printed = obs(SPEKTRA_6MM_SOFTWOOD_POCKET)
        .chipload_max_mm_tooth
        .expect("the Spektra row prints a chip");
    let point = row
        .printed_chipload()
        .point_mm()
        .expect("the Spektra row is a point");
    assert!(
        (point - printed).abs() < 1e-9,
        "the row is exact in diameter and unscaled: {point} vs {printed}"
    );
    // The shipped chip: shipped feed / (shipped RPM x Z). The feed rounds
    // down to 1 mm/min, so the chip sits within 1 / (RPM x Z) below the
    // point.
    let rpm = s
        .operation
        .spindle_rpm()
        .map_or(s.feeds_result.rpm, f64::from);
    let feed = s.operation.feed_rate().expect("Suggest writes the feed");
    let z = f64::from(cell.flutes);
    let chip = feed / (rpm * z);
    let step = 1.0 / (rpm * z);
    assert!(
        chip <= point + 1e-9 && chip >= point - step - 1e-9,
        "shipped chip {chip} (feed {feed} at {rpm} rpm) is not the Spektra point {point}"
    );
    // The G6 ramp reads the same printed cell: the Spektra pocket row.
    match &s.feeds_result.ramp {
        rs_cam_core::feeds::ramp::RampBasis::Sourced { drill, .. } => {
            assert_eq!(drill.side_row, SPEKTRA_6MM_SOFTWOOD_POCKET);
        }
        other @ rs_cam_core::feeds::ramp::RampBasis::NoChip { .. } => {
            panic!("the 6 mm flat carries the G6 ramp claim, got {other:?}")
        }
    }
}

/// The product query of the 6 mm 2-flute end mill, softwood, Pocket, with
/// the tool series set to `series`.
fn softwood_flat_pocket_query(series: Option<&str>) -> LookupQuery {
    use common::feeds_cells::{add_time_op, stock_ctx, tool_of};
    let softwood = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let tool = tool_of(ToolType::EndMill, 6.0, 2);
    let operation = add_time_op(OperationType::Pocket, &stock_ctx());
    let machine = MachineProfile::default();
    let input = feeds_input_for_operation(
        &operation,
        &tool,
        &softwood,
        &machine,
        &EMBEDDED_LUT,
        SpindleStrategy::default(),
    );
    let mut query = to_lookup_query(&input).expect("the Pocket routing answers");
    query.tool_subfamily = series.map(str::to_owned);
    query
}

#[test]
fn a_tool_of_the_zrn_series_reads_its_own_row_g_series() {
    let generic = find_best_row_for_geometry(
        &EMBEDDED_LUT,
        &softwood_flat_pocket_query(None),
        &rs_cam_core::feeds::ToolGeometryHint::Flat,
    )
    .expect("a row serves the generic tool");
    assert_eq!(generic.observation_id, SPEKTRA_6MM_SOFTWOOD_POCKET);

    let series = ToolSeries::ZrnCarving.key();
    let own = find_best_row_for_geometry(
        &EMBEDDED_LUT,
        &softwood_flat_pocket_query(Some(series)),
        &rs_cam_core::feeds::ToolGeometryHint::Flat,
    )
    .expect("a row serves the ZrN tool");
    assert_eq!(own.observation_id, ZRN_6MM_SOFTWOOD_POCKET);
}

/// A 1 mm ball in softwood, Parallel finish: the ZrN 3D profiling chart
/// prints the 1 mm 2-flute row; no standard ball row serves 1 mm with a
/// size basis that is not refused. The generic tool still ships the ZrN
/// row.
#[test]
fn a_cell_that_only_a_specialty_row_serves_still_ships_it_g_series() {
    let fallback = "amana-ball-softwood-parallel-1000-2f-zrn";
    assert_eq!(row_series(obs(fallback)), Some(ToolSeries::ZrnCarving));
    let query = LookupQuery {
        tool_family: ToolFamily::BallNose,
        tool_subfamily: None,
        diameter_mm: 1.0,
        flute_count: 2,
        material_family: MaterialFamily::Softwood,
        hardness_kind: softwood_flat_pocket_query(None).hardness_kind,
        hardness_value: softwood_flat_pocket_query(None).hardness_value,
        operation_family: LutOperationFamily::Parallel,
        pass_role: LutPassRole::Finish,
    };
    let geometry = rs_cam_core::feeds::ToolGeometryHint::Ball;
    // Non-vacuity: the standard rows alone give no served row.
    let standard = standard_lut();
    let std_row = find_best_row_for_geometry(&standard, &query, &geometry);
    assert!(
        std_row.as_ref().is_none_or(|r| r.size_basis.is_refused()),
        "a standard row serves the 1 mm ball: {:?}",
        std_row.map(|r| r.observation_id)
    );
    let row = find_best_row_for_geometry(&EMBEDDED_LUT, &query, &geometry)
        .expect("the specialty row serves the cell");
    assert_eq!(row.observation_id, fallback);
    assert!(!row.size_basis.is_refused());
}

#[test]
fn a_tool_that_declares_a_series_reads_its_own_series_first_g_series() {
    let machine = MachineProfile::default();
    for (direction, series) in [
        (BitCutDirection::Compression, ToolSeries::Compression),
        (BitCutDirection::DownCut, ToolSeries::Downcut),
    ] {
        let mut own_cells = 0_usize;
        let mut failures = Vec::new();
        for cell in common::feeds_cells::walk_with(direction) {
            let Some(s) = cell.suggested.as_ref() else {
                continue;
            };
            let input = feeds_input_for_operation(
                &cell.operation,
                &cell.tool,
                &cell.material,
                &machine,
                &EMBEDDED_LUT,
                SpindleStrategy::default(),
            );
            assert_eq!(
                input.tool_series,
                Some(series),
                "the cut direction declares the series"
            );
            let Some(feed) = s.feeds_result.matched_lut_row.as_ref() else {
                continue;
            };
            let feed_obs = obs(&feed.observation_id);
            if row_series(feed_obs) == Some(series) {
                own_cells += 1;
                continue;
            }
            let Some(query) = to_lookup_query(&input) else {
                continue;
            };
            let tied = lut_where(|o| {
                row_series(o) == Some(series) && ties_on_material_and_size(o, feed_obs)
            });
            if let Some(own) = find_best_row_for_geometry(&tied, &query, &input.tool_geometry)
                && !own.size_basis.is_refused()
            {
                failures.push(format!(
                    "{} ({direction:?}): reads {} while its own series row {} ties on \
                     material and size",
                    cell.label(),
                    feed.observation_id,
                    own.observation_id
                ));
            }
        }
        assert!(own_cells > 0, "no walked cell reads a {series:?} row");
        assert!(
            failures.is_empty(),
            "{} {series:?} cells do not read their own series:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// A compression end mill in MDF, Pocket reads the printed Amana compression
/// row; the generic end mill in the same cell reads a standard row.
#[test]
fn a_compression_tool_reads_the_compression_row_g_series() {
    use common::feeds_cells::{add_time_op, stock_ctx, suggest, tool_of};
    let mdf = Material::SheetGood {
        kind: rs_cam_core::material::SheetGoodKind::Mdf,
    };
    let machine = MachineProfile::default();
    let operation = add_time_op(OperationType::Pocket, &stock_ctx());
    let mut tool = tool_of(ToolType::EndMill, 6.0, 2);
    let generic = suggest(&operation, OperationType::Pocket, &tool, &machine, &mdf)
        .expect("Suggest answers the generic cell");
    let generic_row = generic.feeds_result.matched_lut_row.expect("a row");
    assert!(
        is_standard(obs(&generic_row.observation_id)),
        "the generic tool reads {}",
        generic_row.observation_id
    );
    tool.cut_direction = BitCutDirection::Compression;
    let compression = suggest(&operation, OperationType::Pocket, &tool, &machine, &mdf)
        .expect("Suggest answers the compression cell");
    let row = compression.feeds_result.matched_lut_row.expect("a row");
    assert_eq!(
        row_series(obs(&row.observation_id)),
        Some(ToolSeries::Compression),
        "the compression tool reads {}",
        row.observation_id
    );
}
