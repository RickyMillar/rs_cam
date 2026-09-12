//! U-A1 — `preview_multitool_plan` is the operator's veto, and a veto that
//! costs nothing (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`
//! Phase U item 2, operator decision point §3.1).
//!
//! Two claims, and the second is the one that matters:
//!
//! 1. **The preview is the plan's own arithmetic.** Its tiers are what
//!    [`extract_tier_islands`] over [`compute_tier_map`] gives for the same
//!    ladder and dials — re-derived here from the public API rather than
//!    compared against a recorded number, so this asserts that the session's
//!    resolution reaches the preview, not that one function equals itself.
//! 2. **The preview MUTATES NOTHING.** `&self` says so in the type, but a
//!    type signature is not what an operator is trusting: the project's
//!    serialized form must be byte-identical across the call, and no toolpath
//!    may appear. Rejecting a preview has to be free, or the veto is not a
//!    veto.
//!
//! The fixture is the plan's own: a plate with one spherical bowl tighter
//! than the coarse ball's radius, so the two answers are separable by
//! construction — the plane is tier 0 for both tools, the bowl is territory
//! only the fine tool reaches.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use common::tools::{ball_cutter, ball_tool_config};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::session::{MultitoolPlanSpec, ProjectSession, ProjectSessionBuilder};
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{
    NO_TIER, ResidualTreatment, TierLadder, TierMapParams, compute_tier_map,
};
use rs_cam_core::tool::MillingCutter;

const HALF_MM: f64 = 8.0;
const STEP_MM: f64 = 0.25;
/// Bowl radius (mm) — tighter than the coarse ball's 2.0 mm radius, wider
/// than the fine ball's 1.0 mm.
const BOWL_R_MM: f64 = 1.6;
const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;

fn plane_with_bowl() -> TriangleMesh {
    common::meshes::height_field(HALF_MM, STEP_MM, |x, y| {
        let d_sq = x * x + y * y;
        if d_sq < BOWL_R_MM * BOWL_R_MM {
            -(BOWL_R_MM * BOWL_R_MM - d_sq).sqrt()
        } else {
            0.0
        }
    })
}

/// The bowl is ~8 mm² and the DERIVED floor for an R1.0 tool is
/// `(2·1.0)²·4 = 16 mm²`, which would absorb the fixture's own feature back
/// into the coarse tier. Pinned explicitly — an explicit dial is taken
/// verbatim at every coarseness, by contract.
fn island_params() -> TierIslandParams {
    TierIslandParams {
        min_region_area_mm2: Some(1.0),
        ..TierIslandParams::default()
    }
}

fn spec(coarse_id: usize, fine_id: usize, model_id: usize) -> MultitoolPlanSpec {
    MultitoolPlanSpec {
        setup_index: 0,
        model_id,
        tool_ids: vec![coarse_id, fine_id],
        cell_mm: CELL_MM,
        tolerance_mm: TOLERANCE_MM,
        margin_mm: MARGIN_MM,
        treatment: ResidualTreatment::Raw,
        islands: island_params(),
        ..MultitoolPlanSpec::default()
    }
}

/// A session over the fixture with a Ø4 / Ø2 ball ladder.
/// Returns `(session, coarse_id, fine_id, model_id)`.
fn session_with_ladder() -> (ProjectSession, usize, usize, usize) {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(common::session::stock_under(HALF_MM, 6.0));
    let coarse_idx = builder.add_tool(ball_tool_config(4.0));
    let fine_idx = builder.add_tool(ball_tool_config(2.0));
    let coarse_id = builder.tools()[coarse_idx].id.0;
    let fine_id = builder.tools()[fine_idx].id.0;
    let model_id = builder.add_model(common::session::mesh_model(plane_with_bowl(), "bowl"));
    let session = builder.build();
    (session, coarse_id, fine_id, model_id)
}

fn temp_path(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_mt_preview_{}_{}", std::process::id(), name));
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

#[test]
fn the_preview_reproduces_the_hand_run_tier_islands() {
    let (session, coarse_id, fine_id, model_id) = session_with_ladder();
    let cancel = AtomicBool::new(false);
    let preview = session
        .preview_multitool_plan(&spec(coarse_id, fine_id, model_id), &cancel)
        .expect("a two-ball ladder over a mesh must preview");

    // The oracle: the same public API, over an identically built mesh.
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(4.0);
    let fine = ball_cutter(2.0);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();
    let params = TierMapParams {
        cell_mm: CELL_MM,
        tolerance_mm: TOLERANCE_MM,
        margin_mm: MARGIN_MM,
        treatment: ResidualTreatment::Raw,
    };
    let oracle_cancel = AtomicBool::new(false);
    let map = compute_tier_map(
        &mesh,
        &index,
        &ladder,
        &params,
        &(|| oracle_cancel.load(std::sync::atomic::Ordering::SeqCst)),
    )
    .unwrap();
    let cusp_radii = [coarse.cusp_radius_mm(), fine.cusp_radius_mm()];
    let islands = extract_tier_islands(&map, &island_params(), &cusp_radii).unwrap();

    assert_eq!((preview.map.nx, preview.map.ny), (map.nx, map.ny));
    assert_eq!(preview.map.labels, map.labels, "same labels, cell for cell");
    assert_eq!(preview.map.tier_count, 2);
    assert_eq!(preview.map.treatment, ResidualTreatment::Raw);

    // Non-vacuous: the bowl must actually be somebody's fine territory, or
    // "the two agree" is a comparison of two empty answers.
    assert!(
        preview.map.tier_cell_counts()[1] > 0,
        "the bowl is tighter than the coarse ball — tier 1 must own cells"
    );
    assert!(preview.map.labels.contains(&NO_TIER));

    assert_eq!(preview.islands.per_tier.len(), islands.per_tier.len());
    for (got, want) in preview.islands.per_tier.iter().zip(&islands.per_tier) {
        assert_eq!(got.tier, want.tier);
        assert_eq!(got.islands, want.islands);
        assert_eq!(got.raw_island_count, want.raw_island_count);
        assert_eq!(got.owned_cells, want.owned_cells);
        assert_eq!(got.owned_mask, want.owned_mask);
        assert_eq!(got.machining.len(), want.machining.len());
    }
    assert!(
        preview.islands.total_islands() > 0,
        "an island-free preview would make the ownership comparison vacuous"
    );

    // The ladder the preview reports is the ladder the plan will emit with:
    // coarse -> fine, index-aligned with the labels.
    assert_eq!(preview.tool_ids, vec![coarse_id, fine_id]);
    assert_eq!(preview.cusp_radii_mm, vec![2.0, 1.0]);
    assert_eq!(preview.tool_names.len(), 2);
}

/// The order the caller lists tools in must not reach the preview — the
/// ladder sorts on cusp radius itself, exactly as the plan does, or the
/// operator would veto a tiering the plan then reversed.
#[test]
fn the_preview_sorts_the_ladder_coarse_to_fine_like_the_plan() {
    let (session, coarse_id, fine_id, model_id) = session_with_ladder();
    let cancel = AtomicBool::new(false);
    let reversed = MultitoolPlanSpec {
        tool_ids: vec![fine_id, coarse_id],
        ..spec(coarse_id, fine_id, model_id)
    };
    let preview = session
        .preview_multitool_plan(&reversed, &cancel)
        .expect("preview");
    assert_eq!(preview.tool_ids, vec![coarse_id, fine_id]);
    assert_eq!(preview.cusp_radii_mm, vec![2.0, 1.0]);
}

/// THE VETO SENTRY. A preview that could change the project would make
/// rejecting one cost something, which is the whole point of showing it
/// before generation.
#[test]
fn a_preview_leaves_the_project_byte_identical() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    // A hand-built op, so "no toolpath changed" is a claim about something.
    let tc = common::session::toolpath_config(
        "Operator's own finish",
        OperationConfig::UnifiedFinish(UnifiedFinishConfig::default()),
        fine_id,
        model_id,
    );
    let _ = session.add_toolpath(0, tc).unwrap();

    let before_path = temp_path("before");
    let after_path = temp_path("after");
    session.save(&before_path).expect("save before");
    let before = std::fs::read_to_string(&before_path).expect("read before");
    let before_ops = session.toolpath_configs().len();

    let cancel = AtomicBool::new(false);
    let preview = session
        .preview_multitool_plan(&spec(coarse_id, fine_id, model_id), &cancel)
        .expect("preview");
    assert_eq!(preview.map.tier_count, 2, "the preview really ran");

    session.save(&after_path).expect("save after");
    let after = std::fs::read_to_string(&after_path).expect("read after");

    assert_eq!(
        session.toolpath_configs().len(),
        before_ops,
        "a preview emits no op"
    );
    assert!(
        session
            .toolpath_configs()
            .iter()
            .all(|tc| tc.planner_origin.is_none()),
        "a preview stamps no planner provenance"
    );
    assert_eq!(before, after, "the serialized project must not move");

    cleanup(&before_path);
    cleanup(&after_path);
}

/// The preview holds to the plan's own preconditions, so a spec the plan
/// would refuse is refused BEFORE the operator decides on it rather than
/// after.
#[test]
fn the_preview_refuses_what_the_plan_refuses() {
    let (session, coarse_id, fine_id, model_id) = session_with_ladder();
    let cancel = AtomicBool::new(false);

    let one_tool = MultitoolPlanSpec {
        tool_ids: vec![coarse_id],
        ..spec(coarse_id, fine_id, model_id)
    };
    assert!(session.preview_multitool_plan(&one_tool, &cancel).is_err());

    let bad_setup = MultitoolPlanSpec {
        setup_index: 99,
        ..spec(coarse_id, fine_id, model_id)
    };
    assert!(session.preview_multitool_plan(&bad_setup, &cancel).is_err());

    let bad_model = MultitoolPlanSpec {
        model_id: 404,
        ..spec(coarse_id, fine_id, model_id)
    };
    assert!(session.preview_multitool_plan(&bad_model, &cancel).is_err());
}

/// G-TIERWORKER follow-on (operator-requested skip dial): `tier: 0` on the
/// planned-tier boundary resolves to the COMPLEMENT of the fine tiers'
/// owned islands — the plane stays tier 0's territory, the bowl does not,
/// and an emitted skip-dial plan resolves through the same public seam the
/// GUI worker path uses (`planned_tier_boundary_polys`).
#[test]
fn the_skip_dial_resolves_tier_zero_to_the_complement() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    let plan_spec = MultitoolPlanSpec {
        coarse_skips_fine_islands: true,
        ..spec(coarse_id, fine_id, model_id)
    };
    let outcome = session
        .plan_multitool_finishing(&plan_spec)
        .expect("skip-dial plan emits");

    let cancel = AtomicBool::new(false);
    let tier0_id = outcome.toolpath_ids[0];
    let polys = session
        .planned_tier_boundary_polys(tier0_id, &cancel)
        .expect("tier 0 boundary resolves")
        .expect("tier 0 carries a planned boundary under the skip dial");
    assert!(!polys.is_empty(), "the complement is most of the board");

    let region = rs_cam_core::region_set::RegionSet::from_slice(&polys);
    use rs_cam_core::geo::P2;
    assert!(
        !region.contains(&P2::new(0.0, 0.0)),
        "the bowl centre is fine-tier territory — tier 0 must skip it"
    );
    assert!(
        region.contains(&P2::new(6.0, 6.0)),
        "the flat plane stays tier 0's"
    );

    // The fine tier still resolves to its islands through the same seam,
    // and the two are complementary at the bowl centre.
    let tier1_id = outcome.toolpath_ids[1];
    let fine_polys = session
        .planned_tier_boundary_polys(tier1_id, &cancel)
        .expect("tier 1 boundary resolves")
        .expect("tier 1 carries a planned boundary");
    let fine_region = rs_cam_core::region_set::RegionSet::from_slice(&fine_polys);
    assert!(fine_region.contains(&P2::new(0.0, 0.0)));
}
