//! Tiered-finish plan F3c (`planning/tiered_finish_2026-09-30/PLAN.md`): the
//! tier preview reports, per fine tier, the owned territory the tier's tool
//! reaches only with the body above its flutes. ADVISORY: the territory does
//! not change.
//!
//! # The fixture
//!
//! A straight canyon: flat floor at z = 0, vertical walls of height `H`,
//! plateaus at z = `H` on both sides. The ladder is a Ø10 ball (the coarse
//! tool) over the rivmap100 taper (R1.0 tip, 5.7° half-angle, Ø6 shaft)
//! with a 15 mm flute length and a Ø6 shank above it. The canyon is wider
//! than twice the coarse ball radius, so the coarse tool reaches the floor in
//! the middle and the fine tier owns a strip along each wall.
//!
//! # The expected strip, from the cutter profile
//!
//! A floor cell at distance `d` from a wall. The body above the flutes is the
//! shank: a cylinder of radius `r_body` whose bottom is `L_c` above the tip
//! (the rule `stock::collision` applies). The flutes end at radius
//! `r_contact = width_at_height(L_c)`.
//!
//! - `d < r_contact`: the fluted cone rests on the wall's top edge below
//!   `L_c`, so the body clears the edge.
//! - `r_contact < d < r_body`: either the non-fluted cone rests on the edge,
//!   or the tip is on the floor and the shank overlaps the plateau below its
//!   top. Both are strikes.
//! - `d > r_body`: the shank clears the plateau.
//!
//! So at `H = L_c + 2.8` each wall binds a strip of width `r_body −
//! r_contact` (± 1 cell), and at `H = L_c − 1` nothing binds: the whole
//! wall is below the flute top.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::extrude_profile;
use common::session::mesh_model;
use common::tools::ball_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::maps::tier_islands::TierIslandParams;
use rs_cam_core::maps::tier_map::ResidualTreatment;
use rs_cam_core::session::{MultitoolPlanSpec, MultitoolPreview, ProjectSessionBuilder};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};

/// Flute length of the fine tool (mm), the plan's L_c.
const L_C: f64 = 15.0;
const TIP_DIAMETER: f64 = 2.0;
const HALF_ANGLE_DEG: f64 = 5.7;
const SHAFT_DIAMETER: f64 = 6.0;
const SHANK_DIAMETER: f64 = 6.0;
const COARSE_DIAMETER: f64 = 10.0;
/// Canyon width (mm): over twice the coarse ball radius.
const CANYON_WIDTH: f64 = 24.0;
const PLATEAU_WIDTH: f64 = 10.0;
const LENGTH: f64 = 30.0;
const CELL: f64 = 0.2;

fn taper_config() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER,
        taper_half_angle: HALF_ANGLE_DEG,
        shaft_diameter: SHAFT_DIAMETER,
        cutting_length: L_C,
        shank_diameter: SHANK_DIAMETER,
        shank_length: 20.0,
        holder_diameter: 25.0,
        stickout: 45.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

/// The fine tool's shape, for `width_at_height`.
fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(TIP_DIAMETER, HALF_ANGLE_DEG, SHAFT_DIAMETER, L_C)
}

fn canyon(wall_height: f64) -> rs_cam_core::mesh::TriangleMesh {
    let (w, p) = (CANYON_WIDTH / 2.0, PLATEAU_WIDTH);
    extrude_profile(
        &[
            (-w - p, wall_height),
            (-w, wall_height),
            (-w, 0.0),
            (w, 0.0),
            (w, wall_height),
            (w + p, wall_height),
        ],
        0.0,
        LENGTH,
    )
}

fn preview(wall_height: f64) -> MultitoolPreview {
    let half = CANYON_WIDTH / 2.0 + PLATEAU_WIDTH;
    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: 2.0 * half + 4.0,
        y: LENGTH + 4.0,
        z: wall_height + 2.0,
        origin_x: -half - 2.0,
        origin_y: -2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let coarse = builder.add_tool(ball_tool_config(COARSE_DIAMETER));
    let fine = builder.add_tool(taper_config());
    let coarse_id = builder.tools()[coarse].id.0;
    let fine_id = builder.tools()[fine].id.0;
    let model_id = builder.add_model(mesh_model(canyon(wall_height), "canyon"));
    let session = builder.build();
    let spec = MultitoolPlanSpec {
        model_id,
        tool_ids: vec![coarse_id, fine_id],
        cell_mm: CELL,
        tolerance_mm: 0.05,
        // Raw keeps the labels a pure drop-Z residual; the advisory reads
        // the labels whatever the treatment.
        treatment: ResidualTreatment::Raw,
        islands: TierIslandParams {
            overlap_mm: 0.0,
            ..TierIslandParams::default()
        },
        ..MultitoolPlanSpec::default()
    };
    session
        .preview_multitool_plan(&spec, &AtomicBool::new(false))
        .expect("the canyon previews")
}

/// Binding and owned cells of one middle row, split by wall, as
/// distances (mm) from that wall.
struct RowReading {
    left_binding: Vec<f64>,
    right_binding: Vec<f64>,
    left_owned: Vec<f64>,
}

fn middle_row(p: &MultitoolPreview) -> RowReading {
    let reach = p.flute_reach.first().expect("one fine tier, one reading");
    let set = p
        .islands
        .set_for_tier(1)
        .expect("the fine tier owns the strips");
    let grid = p.map.grid;
    let row = ((LENGTH / 2.0 - grid.origin_y) / grid.cell_mm).round() as usize;
    let w = CANYON_WIDTH / 2.0;
    let mut out = RowReading {
        left_binding: Vec::new(),
        right_binding: Vec::new(),
        left_owned: Vec::new(),
    };
    for col in 0..grid.nx {
        let i = grid.index_of(row, col);
        let x = grid.x_of(col);
        if x.abs() >= w {
            continue;
        }
        if reach.binding_mask[i] {
            if x < 0.0 {
                out.left_binding.push(x + w);
            } else {
                out.right_binding.push(w - x);
            }
        }
        if set.owned_mask[i] && x < 0.0 {
            out.left_owned.push(x + w);
        }
    }
    out
}

#[test]
fn a_wall_taller_than_the_flutes_binds_a_strip_of_the_body_width() {
    let wall = L_C + 2.8;
    let p = preview(wall);
    let reach = p.flute_reach.first().expect("one fine tier, one reading");
    assert_eq!(reach.tier, 1);
    assert!((reach.cutting_length_mm - L_C).abs() < 1e-12);
    let r_body = reach.body_radius_mm.expect("the shank is modelled");
    assert!((r_body - SHANK_DIAMETER / 2.0).abs() < 1e-12);
    assert!((reach.body_z_offset_mm.expect("with its offset") - L_C).abs() < 1e-12);

    let r_contact = taper().width_at_height(L_C);
    assert!(
        r_contact < r_body,
        "the flutes end narrower than the body: {r_contact} vs {r_body}"
    );
    assert!(
        taper().width_at_height(wall) > r_contact,
        "the wall must reach above the flutes"
    );

    let row = middle_row(&p);
    // Non-vacuity: the fine tier owns the ground the strip sits on.
    assert!(
        row.left_owned.iter().any(|&d| d > r_contact && d < r_body),
        "the fine tier must own the strip; owned distances {:?}",
        row.left_owned
    );

    let expected_cells = (r_body - r_contact) / CELL;
    for (side, binding) in [("left", &row.left_binding), ("right", &row.right_binding)] {
        assert!(
            (binding.len() as f64 - expected_cells).abs() <= 1.0,
            "{side} wall: {} binding cells, expected {expected_cells:.2} (+- 1) from \
             r_body {r_body} - r_contact {r_contact:.3}; distances {binding:?}",
            binding.len()
        );
        for &d in binding {
            assert!(
                d > r_contact - CELL && d < r_body + CELL,
                "{side} wall: a binding cell at {d:.2} mm lies outside ({r_contact:.3}, \
                 {r_body}) +- one cell"
            );
        }
    }

    assert!(reach.binds());
    assert!(
        (reach.binding_area_mm2 - reach.binding_cells as f64 * CELL * CELL).abs() < 1e-9,
        "the area is the cells times the cell area"
    );
    let text = reach.to_string();
    assert!(text.contains("tier 1"), "text: {text}");
    assert!(text.contains("15.0 mm flutes"), "text: {text}");
}

#[test]
fn a_wall_below_the_flute_top_binds_nothing() {
    let p = preview(L_C - 1.0);
    let reach = p.flute_reach.first().expect("one fine tier, one reading");
    assert!(
        reach.checked_cells > 0,
        "non-vacuity: the fine tier's cells were checked"
    );
    assert_eq!(
        reach.binding_cells,
        0,
        "a {} mm wall is below the {L_C} mm flutes; binding area {:.2} mm2",
        L_C - 1.0,
        reach.binding_area_mm2
    );
    assert!(!reach.binds());
}

/// The advisory never moves territory: the owned mask is the one the islands
/// report, and the binding mask is a subset of it.
#[test]
fn the_advisory_reads_the_territory_and_does_not_change_it() {
    let p = preview(L_C + 2.8);
    let reach = p.flute_reach.first().unwrap();
    let set = p.islands.set_for_tier(1).unwrap();
    assert_eq!(reach.binding_mask.len(), set.owned_mask.len());
    assert!(
        reach
            .binding_mask
            .iter()
            .zip(set.owned_mask.iter())
            .all(|(&b, &o)| !b || o),
        "every binding cell is an owned cell"
    );
    assert!(reach.checked_cells <= set.owned_cells);
}
