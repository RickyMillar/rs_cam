//! Tier trial (planning/tier_trial_2026-10-01/) — the "blob" preview.
//!
//! Operator idea (2026-10-02): a big ball takes the flats and the sea, and the
//! R1 takes the whole mountain range as one big region. This preview pays
//! only the tier-map walk (no toolpath). For the ladder [Ø6 ball (R3), R1]
//! at tolerance 0.15 it sweeps the island close radius and the minimum
//! island area, and prints the R1 territory: island count and area.
//!
//! Run: `cargo test --release -p rs_cam_core --features test-support --test
//! tier_trial_blob_preview -- --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::tool_config::{
    BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType,
};
use rs_cam_core::session::{AddToolArgs, Command, MultitoolPlanSpec, ProjectSession};

const R1_TOOL: usize = 6;
const BOARD_MM2: f64 = 350.0 * 350.0;

fn r3_ball() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
    t.name = "R3 6mm Ball Nose 2F (HYPOTHETICAL)".to_owned();
    t.diameter = 6.0;
    t.cutting_length = 25.0;
    t.shaft_diameter = 6.0;
    t.shank_diameter = 6.0;
    t.holder_diameter = 25.0;
    t.stickout = 40.0;
    t.flute_count = 2;
    t.tool_material = ToolMaterial::Carbide;
    t.cut_direction = BitCutDirection::UpCut;
    t
}

#[test]
#[ignore = "tier trial blob preview: release build, minutes"]
fn tier_trial_blob_preview() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("planning/fixtures/rivmap100/rivmap100_memory_repro.toml");
    let mut session = ProjectSession::load(&fixture).expect("load the 350 proxy");
    let created = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(r3_ball()),
        }))
        .expect("add the R3 ball")
        .created
        .expect("the new tool index");
    let r3 = session.tools()[created].id.0;
    let cancel = AtomicBool::new(false);

    let tolerance: f64 = std::env::var("BLOB_TOLERANCE")
        .ok()
        .map_or(0.15, |v| v.parse().expect("BLOB_TOLERANCE is a number"));
    eprintln!("ladder [R3 {r3}, R1 {R1_TOOL}], tolerance {tolerance}");
    eprintln!(
        "close_mm  min_area_mm2 | raw_islands after_close after_min kept | \
         owned_mm2 machining_mm2 R1_share  R3_share | holes  dropped_mm2"
    );
    for close in [None, Some(2.0), Some(5.0), Some(10.0), Some(20.0)] {
        for min_area in [None, Some(500.0), Some(2000.0)] {
            let mut spec = MultitoolPlanSpec {
                setup_index: 0,
                model_id: 1,
                tool_ids: vec![r3, R1_TOOL],
                tolerance_mm: tolerance,
                cusp_height_mm: 0.03,
                coarse_skips_fine_islands: true,
                ..MultitoolPlanSpec::default()
            };
            spec.islands.close_radius_mm = close;
            spec.islands.min_region_area_mm2 = min_area;
            let p = session
                .preview_multitool_plan(&spec, &cancel)
                .expect("the preview");
            let set = &p.islands.per_tier[0];
            eprintln!(
                "{:>8} {:>13} | {:>11} {:>11} {:>9} {:>4} | {:>9.0} {:>13.0} {:>8.3} {:>9.3} | {:>5} {:>12.0}",
                close.map_or("def".to_owned(), |c| format!("{c}")),
                min_area.map_or("def".to_owned(), |a| format!("{a}")),
                set.raw_island_count,
                set.cap.islands_after_close,
                set.cap.islands_after_min_area,
                set.cap.kept,
                set.owned_area_mm2,
                set.machining_area_mm2,
                set.machining_area_mm2 / BOARD_MM2,
                1.0 - set.owned_area_mm2 / BOARD_MM2,
                set.owned_hole_count,
                set.cap.dropped_area_mm2,
            );
        }
    }
}
