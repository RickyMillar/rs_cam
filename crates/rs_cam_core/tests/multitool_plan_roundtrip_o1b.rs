//! O-A1 / O-A2 — a planned chain survives project IO, and the two loader
//! doors agree about it.
//!
//! # Why "the doors agree" is the assertion, not "the value is right"
//!
//! A project file stores a toolpath's CONFIG, never its geometry, and more
//! than one code path turns that stored config back into live state. This
//! pair has diverged three times in this repo — most recently G-UNITSRELOAD,
//! where `ModelUnits` was applied by one door and not the other and an
//! inch-authored model reloaded 25.4× smaller in silence. The sentry that
//! closed it (`model_units_survive_reload_g_unitsreload.rs`) asserts the two
//! doors **agree** rather than asserting a size, and this file follows it.
//!
//! The doors reachable from `rs_cam_core` are
//! [`ProjectSession::load`] (file → session) and
//! [`ProjectSession::from_project_file`] (already-parsed → session). Both are
//! exercised on the same bytes. The THIRD door — `rs_cam_viz`'s own project
//! reader — cannot be reached from this crate; its half of the contract is
//! that the keys asserted present below are read there too.
//!
//! # What must survive
//!
//! Both Phase O additions, and both are recipes rather than results:
//!
//! * [`PlannerOrigin`] — without it a re-plan cannot tell its own ops from
//!   the operator's, and `None` on an old file must keep meaning "the
//!   operator built this".
//! * `BoundarySource::PlannedTierRegions` — the FULL ladder, the tier, and
//!   every dial that decides which cells the tier owns. A recipe that lost
//!   its `treatment` would silently re-plan a slope-compensated boundary as
//!   a raw one, which the wanaka A/B measured as the difference between
//!   22.0% and 71.6% of the board.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::{Path, PathBuf};

use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::session::{PlannerOrigin, ProjectFile, ProjectSession};
use rs_cam_core::tier_islands::TierIslandParams;
use rs_cam_core::tier_map::ResidualTreatment;

fn temp_path(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_mt_rt_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
}

/// The recipe under test — every field set to something distinguishable from
/// its default, so a key silently dropped by either door reads as the default
/// rather than coincidentally matching.
fn recipe(tool_ids: Vec<usize>) -> BoundarySource {
    BoundarySource::PlannedTierRegions {
        tool_ids,
        tier: 1,
        cell_mm: 0.37,
        tolerance_mm: 0.041,
        margin_mm: 0.62,
        treatment: ResidualTreatment::SlopeCompensated,
        islands: TierIslandParams {
            close_radius_mm: Some(0.83),
            min_region_area_mm2: Some(17.5),
            coarseness: 1.75,
            overlap_mm: 2.5,
            max_regions_per_tier: 13,
            rim_erosion_mm: 3.25,
        },
    }
}

/// One tool pair, one hand-built op carrying both Phase O additions, and one
/// plain op carrying neither.
fn session_with_a_planned_op() -> (ProjectSession, usize, usize) {
    let mut session = ProjectSession::new_empty();
    let coarse = session
        .add_tool(ball_tool_config(4.0))
        .created
        .expect("add_tool reports the new tool index");
    let fine = session
        .add_tool(ball_tool_config(3.0))
        .created
        .expect("add_tool reports the new tool index");
    let (coarse_id, fine_id) = (session.tools()[coarse].id.0, session.tools()[fine].id.0);

    let plain = common::session::toolpath_config(
        "Hand pass",
        OperationConfig::UnifiedFinish(Default::default()),
        coarse_id,
        0,
    );
    let _ = session.add_toolpath(0, plain).unwrap();

    let mut planned = common::session::toolpath_config(
        "Finish tier 1 (R1.5)",
        OperationConfig::UnifiedFinish(Default::default()),
        fine_id,
        0,
    );
    planned.stock_source = StockSource::FromRemainingStock;
    planned.boundary_inherit = false;
    planned.boundary = BoundaryConfig {
        enabled: true,
        source: recipe(vec![coarse_id, fine_id]),
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    planned.planner_origin = Some(PlannerOrigin {
        plan_id: 7,
        tier: 1,
        tier_count: 2,
    });
    let _ = session.add_toolpath(0, planned).unwrap();
    (session, coarse_id, fine_id)
}

fn assert_matches_original(loaded: &ProjectSession, coarse_id: usize, fine_id: usize, door: &str) {
    let tcs = loaded.toolpath_configs();
    assert_eq!(tcs.len(), 2, "{door}: both ops must survive");
    assert_eq!(
        tcs[0].planner_origin, None,
        "{door}: a hand-built op must NOT acquire provenance on reload"
    );
    assert_eq!(
        tcs[1].planner_origin,
        Some(PlannerOrigin {
            plan_id: 7,
            tier: 1,
            tier_count: 2,
        }),
        "{door}: provenance must survive verbatim"
    );
    assert!(tcs[1].boundary.enabled, "{door}");
    assert_eq!(
        tcs[1].boundary.source,
        recipe(vec![coarse_id, fine_id]),
        "{door}: the whole recipe, field for field — a lost `treatment` or a \
         truncated ladder re-plans a different boundary in silence"
    );
    assert_eq!(
        tcs[1].stock_source,
        StockSource::FromRemainingStock,
        "{door}"
    );
    assert!(!tcs[1].boundary_inherit, "{door}");
}

#[test]
fn both_loader_doors_reproduce_a_planned_chain_identically() {
    let (session, coarse_id, fine_id) = session_with_a_planned_op();
    let path = temp_path("doors");
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();

    // The keys must actually be IN the file. A door that reads them
    // correctly is worthless if the writer never emitted them, and this is
    // the half a reader in another crate depends on.
    assert!(
        text.contains("planner_origin"),
        "the writer must emit the provenance table:\n{text}"
    );
    assert!(
        text.contains("planned_tier_regions"),
        "the writer must emit the boundary source tag:\n{text}"
    );

    // Door A: path -> session.
    let door_a = ProjectSession::load(&path).unwrap();
    // Door B: text -> ProjectFile -> session.
    let parsed: ProjectFile = toml::from_str(&text).unwrap();
    let door_b = ProjectSession::from_project_file(parsed, path.parent().unwrap()).unwrap();

    assert_matches_original(&door_a, coarse_id, fine_id, "door A (load)");
    assert_matches_original(&door_b, coarse_id, fine_id, "door B (from_project_file)");

    // And the doors agree with EACH OTHER, which is the claim that survives
    // even if both are wrong in the same way tomorrow.
    for (a, b) in door_a
        .toolpath_configs()
        .iter()
        .zip(door_b.toolpath_configs())
    {
        assert_eq!(a.planner_origin, b.planner_origin);
        assert_eq!(a.boundary.source, b.boundary.source);
    }
    cleanup(&path);
}

/// A project written before Phase O has neither key. It must load as "the
/// operator built every op" — never as a plan nobody made.
#[test]
fn a_pre_phase_o_project_loads_with_no_provenance() {
    let mut session = ProjectSession::new_empty();
    let _ = session.add_tool(ball_tool_config(3.0));
    let tool_id = session.tools()[0].id.0;
    let plain = common::session::toolpath_config(
        "Hand pass",
        OperationConfig::UnifiedFinish(Default::default()),
        tool_id,
        0,
    );
    let _ = session.add_toolpath(0, plain).unwrap();

    let path = temp_path("legacy");
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("planner_origin"),
        "an absent provenance must be SKIPPED, not written as an empty \
         table — old files stay byte-identical:\n{text}"
    );

    let loaded = ProjectSession::load(&path).unwrap();
    assert_eq!(loaded.toolpath_configs()[0].planner_origin, None);
    cleanup(&path);
}
