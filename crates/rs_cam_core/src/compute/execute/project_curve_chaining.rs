//! Curve chaining for `project_curve` — the gates that decide whether the
//! dial may ship, and what it may never do.
//!
//! ## What was converted, and why this class
//!
//! `project_curve` emits one `rapid → EntryPlunge → cut → Retract` unit per
//! contiguous projected stretch (`Toolpath::emit_path_segment_with_intent`),
//! unconditionally, however short the hop to the next one. A rivers DXF is
//! hundreds of those across dozens of entities. Finishing/engraving air is
//! **count-bound** — every hop pays two full safe-Z legs whatever its XY
//! length — so on this family the round trips, not the cutting, set the wall
//! clock. The Project Curve link body (`ProjectCurveOp::link`, W6.5) applies
//! `surface_link::relink_fragments` to the COMBINED path (not per polygon),
//! so chains from different DXF entities can join.
//!
//! ## What makes this op different from the two that already relink
//!
//! Scallop and unified-finish relink FINISHING passes, where everything
//! above the mesh has already been cleared: the mesh IS the material, so a
//! link that rides the drop-cutter surface rides the workpiece. Engraving
//! does not get that guarantee — `project_curve` routinely runs on raw or
//! partly-roughed stock, and a mesh-riding link there is a cutting feed
//! straight through whatever is standing above the mesh. That is why this
//! op passes `RelinkParams::link_ceiling` and the other two pass `None`, and
//! it is what `stock_safety_*` below exists to hold.
//!
//! ## Gate-design notes inherited from `scallop_intra_pass_relink_am7.rs`
//!
//! * **Never select the population by an intent LABEL.** Wave 11 spent a
//!   whole wave adjudicating 21 "lost cut positions" that were `LeadOut`
//!   endpoints `arcfit::fit_arcs` had relabelled `FinishingCut`. These
//!   sentries select by MOVE TYPE (non-`Rapid`) and compare POSITIONS, and
//!   the one test that runs the dressup stack switches `arc_fitting` and
//!   `lead_in_out` off so no transform in the path relabels anything.
//! * **A gate handed an empty population passes and looks healthy.** Every
//!   assertion below is paired with a non-vacuity control: links > 0,
//!   `outside_boundary` > 0, a refusal count that moves.
//! * The relinker's own structural claims (no fed position dropped, every
//!   input move accounted for by the provenance) are unit-tested in
//!   `surface_link`; these are the ADAPTER-level gates.
//!
//! Moved in-crate in WP12 because the loose entry is crate-private. The lint
//! allows of the old integration header sit on the `mod` declaration in
//! `compute/execute.rs`. The one helper this file took from the integration
//! fixtures, `common::tools::endmill_tool_config`, is inlined below: an
//! in-crate module cannot reach `tests/common/`.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::{DressupConfig, ResolvedHeights};
use crate::compute::cutter::build_cutter;
use crate::compute::execute::{
    ExecutionContext, GenerationFindings, TerritoryInputs, apply_dressups,
    execute_operation_annotated,
};
use crate::compute::operation_configs::{ProjectCurveConfig, ProjectCurveDirection};
use crate::compute::stats::compute_retract_trips;
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::{BoundingBox3, P2, P3};
use crate::geometry::region_set::RegionSet;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use crate::tool::{FlatEndmill, MillingCutter};
use crate::toolpath::{Move, MoveIntent, MoveType, PLUNGE_CLEARANCE_MM, Toolpath};
use crate::trace::toolpath_spans::AnnotatedToolpath;
use crate::trace::transform_provenance::ReconcileSet;

use std::sync::atomic::AtomicBool;

// ── Fixture ──────────────────────────────────────────────────────────────

/// A flat end mill at `diameter_mm`, every other dial at its default.
///
/// This is `common::tools::endmill_tool_config` verbatim. WP12 moved this
/// file in-crate, and an in-crate module cannot reach `tests/common/`, so
/// the four lines live here.
fn endmill_tool_config(diameter_mm: f64) -> ToolConfig {
    ToolConfig::builder(ToolId::mint(0), ToolType::EndMill)
        .diameter(diameter_mm)
        .build()
}

/// Ø2 flat engraver. Small enough that the conservative-ceiling disc stays
/// local to the gap it is asked about.
const TOOL_DIAMETER_MM: f64 = 2.0;
const CUT_DEPTH_MM: f64 = 1.0;
const POINT_SPACING_MM: f64 = 0.5;
/// Every gap in the fixture. Inside the candidate hookup, outside the sample
/// spacing — so a link has interior samples and is not the degenerate case.
const GAP_MM: f64 = 1.5;
const CHAIN_LEN_MM: f64 = 4.0;
const CHAIN_COUNT: usize = 8;
const CHAIN_Y_MM: f64 = 10.0;
const FIRST_CHAIN_X_MM: f64 = 1.0;
/// The candidate. A little over the gap, well under the next chain.
const CANDIDATE_CHAIN_MM: f64 = 3.0;
const SAFE_Z_MM: f64 = 20.0;

const PLATE_X0: f64 = -2.0;
const PLATE_X1: f64 = 48.0;
const PLATE_Y0: f64 = 4.0;
const PLATE_Y1: f64 = 16.0;

/// Start X of chain `i`.
fn chain_x0(i: usize) -> f64 {
    FIRST_CHAIN_X_MM + i as f64 * (CHAIN_LEN_MM + GAP_MM)
}

/// The XY interval of the gap AFTER chain `i` — the ground a link across
/// that junction has to travel over.
fn gap_x(i: usize) -> (f64, f64) {
    (chain_x0(i) + CHAIN_LEN_MM, chain_x0(i + 1))
}

/// A flat plate at z = 0. The projected curves ride it, so every cut sits at
/// exactly `-CUT_DEPTH_MM` and any Z that is not that is link geometry.
fn plate() -> (TriangleMesh, SpatialIndex) {
    let verts = vec![
        P3::new(PLATE_X0, PLATE_Y0, 0.0),
        P3::new(PLATE_X1, PLATE_Y0, 0.0),
        P3::new(PLATE_X1, PLATE_Y1, 0.0),
        P3::new(PLATE_X0, PLATE_Y1, 0.0),
    ];
    let mesh = TriangleMesh::from_raw(verts, vec![[0, 1, 2], [0, 2, 3]]);
    let index = SpatialIndex::build(&mesh, 5.0);
    (mesh, index)
}

/// `CHAIN_COUNT` short open paths along X, each its own `Polygon2` — the
/// multi-entity shape a rivers DXF arrives in, and the reason the relink has
/// to run on the COMBINED path rather than inside the per-polygon loop.
fn curves() -> Vec<Polygon2> {
    (0..CHAIN_COUNT)
        .map(|i| {
            let x0 = chain_x0(i);
            Polygon2::open_path(vec![
                P2::new(x0, CHAIN_Y_MM),
                P2::new(x0 + CHAIN_LEN_MM, CHAIN_Y_MM),
            ])
        })
        .collect()
}

fn heights(safe_z: f64) -> ResolvedHeights {
    ResolvedHeights {
        clearance_z: safe_z + 5.0,
        retract_z: safe_z,
        feed_z: 1.0,
        // Stock top == mesh top: the fallback ceiling a run with no dexel
        // snapshot uses is then the mesh itself, so the stock
        // `None` arms measure the link against the surface and nothing else.
        top_z: 0.0,
        depth_base_z: 0.0,
        bottom_z: -12.0,
        top_pinned: true,
        bottom_pinned: true,
    }
}

fn project_curve_config(chain_distance_mm: f64) -> ProjectCurveConfig {
    ProjectCurveConfig {
        depth: Some(CUT_DEPTH_MM),
        point_spacing: POINT_SPACING_MM,
        feed_rate: Some(800.0),
        plunge_rate: Some(400.0),
        chain_distance_mm,
        ..ProjectCurveConfig::default()
    }
}

/// Run the op through the SAME entry point production uses.
fn generate_with(
    cfg: ProjectCurveConfig,
    safe_z: f64,
    stock: Option<&TriDexelStock>,
    boundary_regions: Option<&[Polygon2]>,
) -> AnnotatedToolpath {
    generate_with_findings(cfg, safe_z, stock, boundary_regions).0
}

/// [`generate_with`], and the findings the run recorded.
fn generate_with_findings(
    cfg: ProjectCurveConfig,
    safe_z: f64,
    stock: Option<&TriDexelStock>,
    boundary_regions: Option<&[Polygon2]>,
) -> (AnnotatedToolpath, GenerationFindings) {
    let (mesh, index) = plate();
    let polys = curves();
    let tool_cfg = endmill_tool_config(TOOL_DIAMETER_MM);
    let tool_def = build_cutter(&tool_cfg).unwrap();
    let h = heights(safe_z);
    let stock_bbox = BoundingBox3 {
        min: P3::new(PLATE_X0, PLATE_Y0, h.bottom_z),
        max: P3::new(PLATE_X1, PLATE_Y1, 0.0),
    };
    let cancel = AtomicBool::new(false);
    let regions = boundary_regions.map(RegionSet::from_slice);
    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        mesh: Some(&mesh),
        index: Some(&index),
        selection: crate::compute::execute::SelectionInputs::from_polygons(polys.as_slice()),
        territory_inputs: TerritoryInputs::default()
            .stock(stock)
            .regions(regions.as_ref()),
        ..ExecutionContext::new(
            &findings,
            &tool_def,
            &tool_cfg,
            &h,
            &[],
            &stock_bbox,
            &cancel,
        )
    };
    // S11: the fixture states the depth and the feeds.
    let mut op = OperationConfig::ProjectCurve(cfg);
    crate::compute::fixture_values::fill_unset(&mut op);
    let generated = execute_operation_annotated(&ctx, &op)
        .expect("project_curve generates on the flat-plate fixture");
    (generated, findings.into_inner())
}

fn generate(chain_distance_mm: f64) -> AnnotatedToolpath {
    generate_with(
        project_curve_config(chain_distance_mm),
        SAFE_Z_MM,
        None,
        None,
    )
}

// ── Measures ─────────────────────────────────────────────────────────────

/// One trip = one maximal contiguous run of `Rapid` moves. The same rule
/// `compute::stats::compute_retract_trips` uses; kept here as an
/// INDEPENDENT reading so the sentry does not take the production channel's
/// word for its own headline number.
fn rapid_runs(moves: &[Move]) -> usize {
    let mut runs = 0;
    let mut in_run = false;
    for m in moves {
        let is_rapid = matches!(m.move_type, MoveType::Rapid);
        if is_rapid && !in_run {
            runs += 1;
        }
        in_run = is_rapid;
    }
    runs
}

/// Every fed position, selected by MOVE TYPE — never by intent. Chaining
/// legitimately turns a chain's `EntryPlunge` into a `Linking` feed to the
/// same point, so an intent-selected population would report the
/// conversion's whole purpose as a defect.
fn fed_positions(moves: &[Move]) -> Vec<P3> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target)
        .collect()
}

fn same_point(a: P3, b: P3) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9
}

fn positions_lost(before: &[P3], after: &[P3]) -> Vec<P3> {
    before
        .iter()
        .filter(|want| !after.iter().any(|got| same_point(*got, **want)))
        .copied()
        .collect()
}

/// Non-rapid `Linking` moves — the link geometry chaining adds. Used only to
/// LOCATE link moves for the safety assertions, never as a population whose
/// membership is itself the claim.
fn link_moves(moves: &[Move]) -> Vec<Move> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid) && m.intent == MoveIntent::Linking)
        .cloned()
        .collect()
}

/// Move-for-move equality: position, kind and intent.
fn identical(a: &Toolpath, b: &Toolpath) -> bool {
    a.moves.len() == b.moves.len()
        && a.moves.iter().zip(b.moves.iter()).all(|(x, y)| {
            same_point(x.target, y.target) && x.move_type == y.move_type && x.intent == y.intent
        })
}

// ── 1. Default OFF is byte-identical ─────────────────────────────────────

/// A config that never saw the field deserializes to the same `0.0` an
/// explicit one sets.
#[test]
fn the_shipped_default_is_off_and_survives_a_config_that_predates_the_field() {
    let legacy: ProjectCurveConfig = serde_json::from_str(
        r#"{"depth":1.0,"point_spacing":0.5,"feed_rate":800.0,"plunge_rate":400.0}"#,
    )
    .expect("a project file written before the field must still load");
    assert_eq!(
        legacy.chain_distance_mm, 0.0,
        "chaining must ship OFF: any other default rewrites every saved \
         project's emitted motion on load"
    );
    assert_eq!(
        ProjectCurveConfig::default().chain_distance_mm,
        0.0,
        "and the constructed default must agree with the serde default"
    );
}

/// At `0.0` the relinker must not run AT ALL — not "run and find nothing".
///
/// The discriminator: `relink_fragments` re-emits every junction from
/// scratch (retract-then-linking rapid, in that order) and drops the
/// generator's own per-polygon closing retract, so even a zero-hookup pass
/// would produce a different move sequence. Comparing against the RAW
/// generator loop — the exact `for poly in polys` the adapter runs — is
/// therefore a proof of absence, not just of equal length.
#[test]
fn chaining_off_emits_exactly_what_the_generator_emits() {
    let (mesh, index) = plate();
    let tool_cfg = endmill_tool_config(TOOL_DIAMETER_MM);
    let tool_def = build_cutter(&tool_cfg).unwrap();
    let cfg = project_curve_config(0.0);

    let params = crate::ops::project_curve::ProjectCurveParams {
        depth: cfg.depth.expect("S11: the fixture sets it"),
        point_spacing: cfg.point_spacing,
        feed_rate: cfg.feed_rate.expect("S11: the fixture sets it"),
        plunge_rate: cfg.plunge_rate.expect("S11: the fixture sets it"),
        safe_z: SAFE_Z_MM,
        direction: crate::ops::project_curve::ProjectDirection::FromAbove,
        tool_radius: tool_def.radius(),
        side: crate::ops::project_curve::ProjectSide::Center,
        projection: crate::ops::project_curve::ProjectProjection::Cutter,
        setup_z_flipped: false,
    };
    let mut raw = Toolpath::new();
    for poly in curves() {
        let tp = crate::interrupt::run_uncancellable(|cancel| {
            crate::ops::project_curve::project_curve_toolpath_with_cancel(
                &poly, &mesh, &index, &tool_def, &params, cancel,
            )
        });
        raw.moves.extend(tp.moves);
    }

    let generated = generate(0.0);
    assert!(
        identical(&raw, &generated.toolpath),
        "chain_distance_mm = 0.0 must leave the emitted motion untouched: \
         {} raw moves vs {} generated",
        raw.moves.len(),
        generated.toolpath.moves.len()
    );
    assert!(
        link_moves(&generated.toolpath.moves).is_empty(),
        "no link geometry may exist with chaining off"
    );
}

// ── 2. The air it is supposed to remove ──────────────────────────────────

/// G-LINKVISIBLE (D2): the curve runner records the chain-link totals
/// only when the link stage ran. With chaining off nothing is recorded;
/// with chaining on, the totals count every fragment and the links.
#[test]
fn the_chain_link_totals_are_recorded_only_when_the_stage_ran() {
    let (_, off) = generate_with_findings(project_curve_config(0.0), SAFE_Z_MM, None, None);
    assert!(off.relink.is_none(), "no stage, no link totals");

    let (_, on) = generate_with_findings(
        project_curve_config(CANDIDATE_CHAIN_MM),
        SAFE_Z_MM,
        None,
        None,
    );
    let totals = on.relink.expect("a stage that ran records its totals");
    assert_eq!(
        totals.fragments, CHAIN_COUNT,
        "one fragment per projected chain"
    );
    assert!(
        totals.surface_links + totals.retract_links > 0,
        "the stage decided at least one junction: {totals:?}"
    );
}

/// Two independent readings of the same claim: the production channel
/// (`compute_retract_trips`, what the diagnostics surfaces report) and a
/// counter written here from the move list. If the two ever disagree, one of
/// them has stopped measuring what its name says.
#[test]
fn chaining_removes_retract_trips_on_both_channels() {
    let off = generate(0.0);
    let on = generate(CANDIDATE_CHAIN_MM);

    let off_prod = compute_retract_trips(&off.toolpath, None).total;
    let on_prod = compute_retract_trips(&on.toolpath, None).total;
    let off_local = rapid_runs(&off.toolpath.moves);
    let on_local = rapid_runs(&on.toolpath.moves);

    println!(
        "project_curve chaining, {CHAIN_COUNT} chains @ {GAP_MM} mm gaps, \
         cap {CANDIDATE_CHAIN_MM} mm\n  \
         retract trips (production) {off_prod} -> {on_prod}\n  \
         rapid runs   (in-test)     {off_local} -> {on_local}\n  \
         rapid mm     {:.1} -> {:.1}",
        off.toolpath.total_rapid_distance(),
        on.toolpath.total_rapid_distance()
    );

    assert_eq!(
        off_prod, off_local,
        "the production retract-trip channel and this file's own counter \
         must agree on the BASELINE before either is used as evidence"
    );
    assert_eq!(
        on_prod, on_local,
        "…and on the chained path: {off_prod}/{off_local} -> {on_prod}/{on_local}"
    );
    assert!(
        on_prod < off_prod,
        "chaining must remove retract round trips; {off_prod} -> {on_prod}"
    );
    assert!(
        on.toolpath.total_rapid_distance() < off.toolpath.total_rapid_distance(),
        "and the rapid travel with them"
    );
}

// ── 3. Nothing that was cut stops being cut ──────────────────────────────

/// The structural claim the conversion rests on: `relink_fragments` copies
/// fragment INTERIORS verbatim and rewrites only the airborne junctions, so
/// every position the tool fed to before is a position it feeds to after.
///
/// Set-membership, exact, order-independent — a reorder passes, a hole does
/// not. Nothing in this path relabels anything (no dressups run here; the
/// one test that runs them switches `arc_fitting` off), so the comparison
/// means what it says.
#[test]
fn chaining_loses_no_cut_position() {
    let off = generate(0.0);
    let on = generate(CANDIDATE_CHAIN_MM);

    let before = fed_positions(&off.toolpath.moves);
    let after = fed_positions(&on.toolpath.moves);
    let lost = positions_lost(&before, &after);
    let links = link_moves(&on.toolpath.moves).len();

    println!(
        "project_curve chaining: {} fed positions -> {} ({links} link moves), \
         lost {}",
        before.len(),
        after.len(),
        lost.len()
    );
    assert!(
        links > 0,
        "control: the fixture must actually chain, or this gate proves nothing"
    );
    assert!(
        lost.is_empty(),
        "a chain may ADD link feeds; it may never drop a fed position — \
         {} lost, first {:?}",
        lost.len(),
        lost.first()
    );
    // The cut itself is at one known depth on this plate, so a link that
    // silently deepened a cut would show up here and nowhere else.
    for p in &after {
        assert!(
            p.z >= -CUT_DEPTH_MM - 1e-9,
            "no move may go BELOW the commanded engrave depth: {p:?}"
        );
    }
}

// ── 4. Stock safety ──────────────────────────────────────────────────────

const STOCK_CELL_MM: f64 = 0.25;
/// How high the rib stands above the mesh in the stock-safety fixtures.
const RIB_TOP_Z: f64 = 3.0;
/// The junction the rib crosses.
const RIB_GAP_INDEX: usize = 3;

/// Stock that stands `RIB_TOP_Z` above the mesh, then has everything cleared
/// back down to the mesh EXCEPT a full-width wall across one gap.
///
/// Built by stamping (the only way to remove material from a dexel grid):
/// raster the clearing tool along Y at `tip_z = 0`, skipping the columns
/// whose swept disc would eat the wall. `keep_rib = false` clears everything
/// — the control arm, identical in every other respect.
fn ribbed_stock(keep_rib: bool) -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(
        PLATE_X0,
        PLATE_Y0,
        PLATE_X1,
        PLATE_Y1,
        -12.0,
        RIB_TOP_Z,
        STOCK_CELL_MM,
    );
    let clearer = FlatEndmill::new(3.0, 25.0);
    let radius = 1.5;
    let lut = RadialProfileLUT::from_cutter(&clearer, LUT_SAMPLES);
    let (rib_lo, rib_hi) = gap_x(RIB_GAP_INDEX);

    let mut x = PLATE_X0 - radius;
    while x <= PLATE_X1 + radius {
        // A pass at this X sweeps a disc of `radius`, so it reaches
        // `radius` beyond its own line. Skipping the lines within `radius`
        // of the rib leaves exactly the rib standing.
        let clears_rib = keep_rib && x > rib_lo - radius && x < rib_hi + radius;
        if !clears_rib {
            stock.stamp_linear_segment(
                &lut,
                radius,
                P3::new(x, PLATE_Y0 - radius, 0.0),
                P3::new(x, PLATE_Y1 + radius, 0.0),
                StockCutDirection::FromTop,
            );
        }
        x += radius * 0.5;
    }
    stock
}

/// The conservative ceiling the production code reads, restated here so the
/// assertion is against the same query and not a re-derivation of it.
fn ceiling_at(stock: &TriDexelStock, x: f64, y: f64) -> f64 {
    stock
        .max_conservative_top_z_in_disc(x, y, TOOL_DIAMETER_MM * 0.5)
        .unwrap_or(0.0)
}

/// THE gate this op needed and the two finishing families did not.
///
/// A link that rode the mesh across the rib would be a cutting feed through
/// 3 mm of standing material. Every link sample must sit at or above the
/// conservative ceiling; the two exceptions are the link's own endpoints,
/// which ARE cut positions and are where the tool legitimately is.
#[test]
fn stock_safety_links_clear_standing_material() {
    let stock = ribbed_stock(true);
    let on = generate_with(
        project_curve_config(CANDIDATE_CHAIN_MM),
        SAFE_Z_MM,
        Some(&stock),
        None,
    );
    let off = generate_with(project_curve_config(0.0), SAFE_Z_MM, Some(&stock), None);

    let cut_positions = fed_positions(&off.toolpath.moves);
    let links = link_moves(&on.toolpath.moves);
    assert!(
        !links.is_empty(),
        "control: the fixture must chain, or there is nothing to check"
    );

    let mut violations = 0usize;
    let mut worst = 0.0f64;
    let mut highest_link_z = f64::NEG_INFINITY;
    let radius = TOOL_DIAMETER_MM * 0.5;
    let flat_cutter = FlatEndmill::new(TOOL_DIAMETER_MM, 25.0);
    for m in &links {
        // A link ENDS on the next chain's entry — a cut position, below the
        // ceiling by construction. Everything else is traverse geometry.
        if cut_positions.iter().any(|p| same_point(*p, m.target)) {
            continue;
        }
        highest_link_z = highest_link_z.max(m.target.z);
        // The contract is the ceiling PLUS the plunge clearance, not merely
        // "not inside the material" — a link grazing the top of a rib is a
        // cutting feed too.
        // The production ceiling is PROFILE-AWARE since the link-ceiling
        // change: `max over r of [material_top(r) - height_at_radius(r)]`
        // rather than `max over the disc of material_top`. This op runs a FLAT
        // endmill, whose `height_at_radius` is `Some(0.0)` throughout its
        // envelope, so the two must be BYTE-IDENTICAL here — that is the
        // safety anchor, asserted on the very samples this gate grades rather
        // than argued in a comment.
        assert_eq!(
            stock.max_clearance_tip_z_for_profile(m.target.x, m.target.y, radius, &flat_cutter),
            stock.max_conservative_top_z_in_disc(m.target.x, m.target.y, radius),
            "the profile-aware ceiling moved for a FLAT endmill at ({:.3}, \
             {:.3}) — it must reduce to the flat disc exactly",
            m.target.x,
            m.target.y
        );
        let required = ceiling_at(&stock, m.target.x, m.target.y) + PLUNGE_CLEARANCE_MM;
        if m.target.z < required - 1e-9 {
            violations += 1;
            worst = worst.max(required - m.target.z);
        }
    }
    println!(
        "stock safety: {} link moves, {} traverse samples below the ceiling, \
         worst {worst:.3} mm; highest link Z {highest_link_z:.3}",
        links.len(),
        violations
    );
    assert_eq!(
        violations, 0,
        "a link sample below the standing-material ceiling is a fed move \
         THROUGH stock: {violations} of them, worst {worst:.3} mm under"
    );
    // Non-vacuity: at least one link had to climb over the rib. Without the
    // ceiling every one of these would sit at the mesh (z = 0).
    assert!(
        highest_link_z >= RIB_TOP_Z + PLUNGE_CLEARANCE_MM - 1e-6,
        "the rib must actually be crossed by a link — highest link Z was \
         {highest_link_z:.3}, expected at least {:.3}",
        RIB_TOP_Z + PLUNGE_CLEARANCE_MM
    );
}

/// The other half: when clearing the standing material would put the link at
/// or above safe Z there is nothing left for a fed move to save, so the link
/// is refused and the retract kept.
///
/// Measured against a control arm with the SAME safe Z and the SAME stock
/// construction minus the rib, so the one extra retract is attributable to
/// the ceiling and to nothing else.
#[test]
fn stock_safety_links_are_refused_when_the_ceiling_reaches_safe_z() {
    // Rib ceiling 3.0 + 2.0 clearance = 5.0, at or above this safe Z.
    // The mesh elsewhere is 0.0 + 2.0 = 2.0, comfortably below it.
    let tight_safe_z = 4.5;
    let ribbed = ribbed_stock(true);
    let flat = ribbed_stock(false);

    let with_rib = generate_with(
        project_curve_config(CANDIDATE_CHAIN_MM),
        tight_safe_z,
        Some(&ribbed),
        None,
    );
    let without_rib = generate_with(
        project_curve_config(CANDIDATE_CHAIN_MM),
        tight_safe_z,
        Some(&flat),
        None,
    );

    let rib_trips = rapid_runs(&with_rib.toolpath.moves);
    let flat_trips = rapid_runs(&without_rib.toolpath.moves);
    println!(
        "ceiling-vs-safe-Z refusal @ safe_z {tight_safe_z}: rapid runs \
         {flat_trips} (no rib) -> {rib_trips} (rib)"
    );
    assert_eq!(
        rib_trips,
        flat_trips + 1,
        "exactly the one junction the rib crosses must fall back to a \
         retract; every other junction still links"
    );

    // And the retract that came back is at the rib, not somewhere else.
    let (rib_lo, rib_hi) = gap_x(RIB_GAP_INDEX);
    let retracted_over_rib = with_rib.toolpath.moves.iter().any(|m| {
        matches!(m.move_type, MoveType::Rapid)
            && m.target.x >= rib_lo - 1e-6
            && m.target.x <= rib_hi + 1e-6
    });
    assert!(
        retracted_over_rib,
        "the refused junction must be the one spanning the rib ({rib_lo}..{rib_hi})"
    );
    // No link may sit at or above the retract plane — that is the condition
    // the refusal exists to enforce.
    for m in link_moves(&with_rib.toolpath.moves) {
        assert!(
            m.target.z < tight_safe_z - 1e-6,
            "a kept link at or above safe Z is strictly worse than the rapid \
             it replaced: {:?}",
            m.target
        );
    }
}

// ── 5. Boundary containment ──────────────────────────────────────────────

/// Rectangles covering each chain but NOT the gaps between them, so every
/// candidate link crosses excluded territory.
fn chain_only_regions() -> Vec<Polygon2> {
    (0..CHAIN_COUNT)
        .map(|i| {
            let x0 = chain_x0(i);
            Polygon2::new(vec![
                P2::new(x0 - 0.2, CHAIN_Y_MM - 1.0),
                P2::new(x0 + CHAIN_LEN_MM + 0.2, CHAIN_Y_MM - 1.0),
                P2::new(x0 + CHAIN_LEN_MM + 0.2, CHAIN_Y_MM + 1.0),
                P2::new(x0 - 0.2, CHAIN_Y_MM + 1.0),
            ])
        })
        .collect()
}

/// A link is a CUTTING feed: one that leaves the operation's territory
/// machines ground the boundary deliberately excluded — the
/// selective-finishing gouge class `RelinkParams::boundary` exists to
/// prevent.
///
/// Adjudicated twice, because the adapter cannot hand back the
/// `RelinkReport`: once through the production channel (with a boundary the
/// links stop happening) and once by running `relink_fragments` with the
/// same parameters so the `outside_boundary` counter itself is read. The
/// second is what stops the first from passing for the wrong reason.
#[test]
fn a_link_may_not_leave_the_machining_boundary() {
    let regions = chain_only_regions();
    let unbounded = generate(CANDIDATE_CHAIN_MM);
    let bounded = generate_with(
        project_curve_config(CANDIDATE_CHAIN_MM),
        SAFE_Z_MM,
        None,
        Some(regions.as_slice()),
    );

    let free_links = link_moves(&unbounded.toolpath.moves).len();
    let bounded_links = link_moves(&bounded.toolpath.moves).len();
    println!("boundary: {free_links} link moves free -> {bounded_links} bounded");
    assert!(
        free_links > 0,
        "control: without a boundary the fixture chains"
    );
    assert_eq!(
        bounded_links, 0,
        "every junction crosses excluded territory, so every link must be \
         refused"
    );

    // No link sample anywhere outside the region set — stated positively so
    // a future partial-refusal still has to hold the line.
    let region_set = RegionSet::from_slice(&regions);
    for m in link_moves(&bounded.toolpath.moves) {
        assert!(
            region_set.contains(&P2::new(m.target.x, m.target.y)),
            "link sample outside the machining boundary: {:?}",
            m.target
        );
    }

    // The counter itself, read off the library the adapter drives.
    let (mesh, index) = plate();
    let tool_cfg = endmill_tool_config(TOOL_DIAMETER_MM);
    let tool_def = build_cutter(&tool_cfg).unwrap();
    let params = crate::finish::surface_link::RelinkParams {
        link_tolerance: None,
        hookup_distance: CANDIDATE_CHAIN_MM,
        stock_to_leave: 0.0,
        sampling: POINT_SPACING_MM,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        safe_z: SAFE_Z_MM,
        link_kinematics: None,
        reorder: true,
        boundary: Some(&region_set),
        // The production builder (R1), with this fixture's stock top (0).
        link_ceiling: crate::compute::execute::link_ceiling_for(
            OperationType::ProjectCurve,
            None,
            &tool_def,
            0.0,
            heights(SAFE_Z_MM).top_z,
            None,
        ),
        flush_ride: false,
        // project_curve's own prior, mirrored: its boundary is machining
        // territory (keep-outs, fixtures), not a cut-confinement polygon, so
        // an airborne link gets no exemption from it.
        airborne_links_may_leave_territory: false,
    };
    let (_, report) = crate::finish::surface_link::relink_fragments(
        AnnotatedToolpath::new(unbounded_baseline()),
        &mesh,
        &index,
        &tool_def,
        &params,
    );
    println!("boundary report: {report:?}");
    assert!(
        report.outside_boundary > 0,
        "the boundary check must have FIRED, not merely coincided with a \
         path that had no links to lose: {report:?}"
    );
    assert_eq!(
        report.surface_links, 0,
        "…and nothing may have slipped past it: {report:?}"
    );
}

/// The unchained path, as a bare `Toolpath`, for the direct-library arm
/// above.
fn unbounded_baseline() -> Toolpath {
    generate(0.0).toolpath
}

// ── 6. Composition with the rapid-order TSP ──────────────────────────────

/// `tsp::optimize_rapid_order` splits only at RAPIDS, and a chained junction
/// is a run of `Linking` FEED moves — so the two passes agree on what is
/// atomic and a chained junction cannot be torn apart by the reorder.
///
/// `arc_fitting` and `lead_in_out` are off: both relabel geometry, and a
/// position comparison over a relabelled population is exactly the reading
/// that cost wave 11 a whole wave.
#[test]
fn chained_junctions_survive_the_rapid_order_dressup() {
    let on = generate(CANDIDATE_CHAIN_MM);
    let before = on.toolpath.clone();

    let dressups = DressupConfig {
        optimize_rapid_order: true,
        arc_fitting: None,
        lead_in_out: None,
        link_moves: None,
        segment_merge: None,
        feed_optimization: false,
        ..DressupConfig::for_op(OperationType::ProjectCurve)
    };
    let tool_cfg = endmill_tool_config(TOOL_DIAMETER_MM);
    let tool_def = build_cutter(&tool_cfg).unwrap();
    let after = apply_dressups(
        on,
        crate::compute::execute::DressupContext {
            material: &crate::material::Material::default(),
            ramp_feed_rate_mm_min: None,
            cfg: &dressups,
            nominal_feed_rate: 800.0,
            // P29-8 8c: the dressup door needs the op plunge rate; this fixture
            // plunges at its cut feed.
            plunge_rate_mm_min: 800.0,
            tool_diameter: TOOL_DIAMETER_MM,
            safe_z: SAFE_Z_MM,
            stock_top: 0.0,
            prior_stock: None,
            link_kinematics: None,
            op_cutting_ceiling: None,
            feed_opt_stock: None,
            cutter: Some(&tool_def as &dyn MillingCutter),
            entry_surface: None,
            transform_capabilities: OperationType::ProjectCurve.capabilities().transform,
            entry: OperationType::ProjectCurve.registry_entry().entry,
            debug_ctx: None,
            semantic_ctx: None,
            entry_containment: None,
        },
        &mut ReconcileSet::empty(),
    );

    // A junction is the ORDERED PAIR (previous target -> link target). If
    // the TSP split one, the pair disappears.
    let junction_pairs = |tp: &Toolpath| -> Vec<(String, String)> {
        tp.moves
            .windows(2)
            .filter(|w| {
                !matches!(w[1].move_type, MoveType::Rapid) && w[1].intent == MoveIntent::Linking
            })
            .map(|w| {
                let fmt = |p: P3| format!("{:.4},{:.4},{:.4}", p.x, p.y, p.z);
                (fmt(w[0].target), fmt(w[1].target))
            })
            .collect()
    };
    let mut want = junction_pairs(&before);
    let mut got = junction_pairs(&after.toolpath);
    println!(
        "TSP composition: {} link junction pairs before, {} after",
        want.len(),
        got.len()
    );
    assert!(
        !want.is_empty(),
        "control: there must be chained junctions for the TSP to threaten"
    );
    want.sort();
    got.sort();
    assert_eq!(
        want, got,
        "the rapid-order TSP splits only at rapids, so every chained \
         junction — a run of Linking FEED moves — must survive it intact"
    );
}

// ── 7. The frame guard ───────────────────────────────────────────────────

/// Standalone `FromBelow` projects against a mesh it Z-flips internally and
/// flips the result back, so a relink costed against `ctx.mesh` would sample
/// the wrong surface. The adapter refuses to chain there — it emits the
/// unchained path rather than geometry from two frames.
///
/// (The session never reaches this: on a bottom-facing setup the runner
/// reads a flipped frame from `ExecutionContext::z_flipped`, which takes the
/// other arm. This context has no setup transform, so the frame is not
/// flipped.)
#[test]
fn standalone_from_below_refuses_to_chain() {
    let mut cfg = project_curve_config(CANDIDATE_CHAIN_MM);
    cfg.direction = ProjectCurveDirection::FromBelow;
    let chained = generate_with(cfg.clone(), SAFE_Z_MM, None, None);

    let mut off = cfg;
    off.chain_distance_mm = 0.0;
    let unchained = generate_with(off, SAFE_Z_MM, None, None);

    assert!(
        link_moves(&chained.toolpath.moves).is_empty(),
        "standalone FromBelow must emit no link geometry"
    );
    assert!(
        identical(&chained.toolpath, &unchained.toolpath),
        "…and must be move-for-move what the unchained pass emits"
    );
}

// ── 8. P33: the depth ladder ─────────────────────────────────────────────

/// The config of the ladder tests, read through serde. An op table takes an
/// unknown key, so the red test compiled before the key existed.
fn ladder_config(depth: f64, depth_per_pass: Option<f64>) -> ProjectCurveConfig {
    let mut value = serde_json::json!({
        "depth": depth,
        "point_spacing": POINT_SPACING_MM,
        "feed_rate": 800.0,
        "plunge_rate": 400.0,
    });
    if let Some(step) = depth_per_pass {
        value["depth_per_pass"] = step.into();
    }
    serde_json::from_value(value).expect("the ladder config loads")
}

/// The cut Z of each fed chain, in the emitted order: the Z of the first
/// fed move after each rapid run.
fn chain_cut_zs(moves: &[Move]) -> Vec<f64> {
    moves
        .windows(2)
        .filter(|w| {
            matches!(w[0].move_type, MoveType::Rapid) && !matches!(w[1].move_type, MoveType::Rapid)
        })
        .map(|w| w[1].target.z)
        .collect()
}

/// P33 (operator ruling 2026-11-15): a depth per pass gives the passes
/// inside ONE toolpath. The plate top is Z 0 = the op top Z. Depth 3.0 at
/// 1.0 per pass: the one ladder (`ops::depth::z_ladder`, the 2.5D arm)
/// gives the levels -1, -2, -3. Each pass covers every curve (pass-major),
/// the last pass is the path of an op with no key, and no pass cuts more
/// than one step.
///
/// RED before the code: the key did not exist, so the op cut one pass at
/// -3.0 (8 chains, not 24).
#[test]
fn a_depth_per_pass_cuts_the_curve_in_passes_p33() {
    let laddered = generate_with(ladder_config(3.0, Some(1.0)), SAFE_Z_MM, None, None);
    let one_pass = generate_with(ladder_config(3.0, None), SAFE_Z_MM, None, None);

    let zs = chain_cut_zs(&laddered.toolpath.moves);
    let want: Vec<f64> = [-1.0, -2.0, -3.0]
        .into_iter()
        .flat_map(|z| std::iter::repeat_n(z, CHAIN_COUNT))
        .collect();
    assert_eq!(
        zs.len(),
        want.len(),
        "3 passes over {CHAIN_COUNT} chains: {zs:?}"
    );
    for (got, want) in zs.iter().zip(&want) {
        assert!(
            (got - want).abs() < 1e-9,
            "pass depths {zs:?}, want {want:?}"
        );
    }
    // The last pass is the path of the op with no key, move for move.
    let tail = laddered.toolpath.moves.len() - one_pass.toolpath.moves.len();
    let last_pass = Toolpath {
        moves: laddered.toolpath.moves[tail..].to_vec(),
    };
    assert!(
        identical(&last_pass, &one_pass.toolpath),
        "the last pass is the full depth, the path of today"
    );
    // No rapid goes below the retract plane, and every pass enters as the
    // first pass does: a rapid at the retract plane, then one plunge.
    for m in &laddered.toolpath.moves {
        if matches!(m.move_type, MoveType::Rapid) {
            assert!(
                (m.target.z - SAFE_Z_MM).abs() < 1e-9,
                "a rapid at Z {} is not at the retract plane",
                m.target.z
            );
        }
    }
    let plunges = laddered
        .toolpath
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .count();
    assert_eq!(
        plunges,
        3 * CHAIN_COUNT,
        "one straight plunge per chain and pass"
    );
}

/// A solid block under the plate top (Z 0), cell 0.25 mm: the stock that
/// the ladder cuts into.
fn block_stock() -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(PLATE_X0, PLATE_Y0, -12.0),
            max: P3::new(PLATE_X1, PLATE_Y1, 0.0),
        },
        0.25,
    )
}

/// The registry dressups of a Project Curve on `stock`, as the session
/// applies them after generation.
fn dressed(generated: AnnotatedToolpath, stock: &TriDexelStock) -> AnnotatedToolpath {
    let dressups = DressupConfig::for_op(OperationType::ProjectCurve);
    let tool_cfg = endmill_tool_config(TOOL_DIAMETER_MM);
    let tool_def = build_cutter(&tool_cfg).unwrap();
    apply_dressups(
        generated,
        crate::compute::execute::DressupContext {
            material: &crate::material::Material::default(),
            ramp_feed_rate_mm_min: None,
            cfg: &dressups,
            nominal_feed_rate: 800.0,
            plunge_rate_mm_min: 400.0,
            tool_diameter: TOOL_DIAMETER_MM,
            safe_z: SAFE_Z_MM,
            stock_top: 0.0,
            prior_stock: Some(stock),
            link_kinematics: None,
            op_cutting_ceiling: None,
            feed_opt_stock: None,
            cutter: Some(&tool_def as &dyn MillingCutter),
            entry_surface: None,
            transform_capabilities: OperationType::ProjectCurve.capabilities().transform,
            entry: OperationType::ProjectCurve.registry_entry().entry,
            debug_ctx: None,
            semantic_ctx: None,
            entry_containment: None,
        },
        &mut ReconcileSet::empty(),
    )
}

/// The lowest Z of each run of fed moves between two rapids, in order.
fn run_floor_zs(moves: &[Move]) -> Vec<f64> {
    let mut floors: Vec<f64> = Vec::new();
    let mut open = false;
    for m in moves {
        if matches!(m.move_type, MoveType::Rapid) {
            open = false;
            continue;
        }
        if open {
            if let Some(floor) = floors.last_mut() {
                *floor = floor.min(m.target.z);
            }
        } else {
            floors.push(m.target.z);
            open = true;
        }
    }
    floors
}

/// P33: the ladder holds after the dressup stage of the session (the
/// registry dressups of the op, the rapid-order dressup with them, on
/// solid stock). The passes stay in order: the rapid-order dressup moves
/// no chain to another pass (a barrier before each later pass). No rapid
/// ends inside the stock.
#[test]
fn the_ladder_keeps_its_order_and_its_rapids_after_the_dressups_p33() {
    let stock = block_stock();
    let generated = generate_with(ladder_config(3.0, Some(1.0)), SAFE_Z_MM, Some(&stock), None);
    assert_eq!(
        generated.rapid_order_barriers().len(),
        2,
        "one barrier before pass 2 and one before pass 3"
    );
    assert!(
        DressupConfig::for_op(OperationType::ProjectCurve).optimize_rapid_order,
        "non-vacuity: the registry dressups hold the rapid-order dressup"
    );
    let after = dressed(generated, &stock);

    let floors = run_floor_zs(&after.toolpath.moves);
    assert_eq!(floors.len(), 3 * CHAIN_COUNT, "floors {floors:?}");
    for (i, floor) in floors.iter().enumerate() {
        let want = -1.0 - (i / CHAIN_COUNT) as f64;
        assert!(
            (floor - want).abs() < 1e-9,
            "run {i} cuts at {floor}, want {want}: a chain left its pass ({floors:?})"
        );
    }
    for m in &after.toolpath.moves {
        if matches!(m.move_type, MoveType::Rapid) {
            assert!(
                m.target.z >= -1e-9,
                "a rapid ends at Z {} inside the stock (top Z 0)",
                m.target.z
            );
        }
    }
}

/// P33: with a chain distance, each pass is linked alone. The link stage
/// re-orders the fragments of ONE pass; no link joins two passes, and the
/// passes stay in order.
#[test]
fn the_link_stage_runs_on_each_pass_alone_p33() {
    let mut cfg = ladder_config(3.0, Some(1.0));
    cfg.chain_distance_mm = CANDIDATE_CHAIN_MM;
    let (laddered, findings) = generate_with_findings(cfg, SAFE_Z_MM, None, None);
    let mut one = ladder_config(3.0, None);
    one.chain_distance_mm = CANDIDATE_CHAIN_MM;
    let (one_pass, one_findings) = generate_with_findings(one, SAFE_Z_MM, None, None);

    let links_one = link_moves(&one_pass.toolpath.moves).len();
    assert!(links_one > 0, "non-vacuity: the one pass has links");
    assert_eq!(
        link_moves(&laddered.toolpath.moves).len(),
        3 * links_one,
        "each of the 3 passes has the links of the one pass"
    );
    let surface = |f: &GenerationFindings| f.relink.map(|totals| totals.surface_links);
    assert_eq!(
        surface(&findings),
        surface(&one_findings).map(|n| 3 * n),
        "the recorded totals are the sum of the passes"
    );
    // The cut positions go down pass by pass: every fed position at the
    // cut depth of a pass comes before every one of the next pass.
    let cut_zs: Vec<f64> = fed_positions(&laddered.toolpath.moves)
        .iter()
        .map(|p| p.z)
        .filter(|z| {
            [-1.0, -2.0, -3.0]
                .iter()
                .any(|level| (z - level).abs() < 1e-9)
        })
        .collect();
    assert!(
        cut_zs.windows(2).all(|w| w[1] <= w[0] + 1e-9),
        "a deeper pass comes before a shallower pass"
    );
    assert!(cut_zs.iter().any(|z| (z + 1.0).abs() < 1e-9));
}

/// P33: the stated cut and the feeds role of the key.
#[test]
fn the_stated_cut_is_one_pass_and_no_feeds_door_starts_a_ladder_p33() {
    use crate::compute::catalog::Axial;

    let mut keyed = OperationConfig::ProjectCurve(ladder_config(3.0, Some(1.0)));
    assert!(
        matches!(keyed.cut_spec().axial, Axial::PerPass(Some(_))),
        "with the key the stated depth is the depth of one pass"
    );
    assert_eq!(keyed.cut_spec().axial.stated_mm(), Some(1.0));
    assert!(!keyed.cut_spec().axial.depth_after_simulation());
    assert_eq!(keyed.depth_per_pass(), Some(1.0));
    assert!(
        keyed.set_depth_per_pass(0.5),
        "a stated key takes a feeds write"
    );
    assert_eq!(keyed.depth_per_pass(), Some(0.5));

    let mut bare = OperationConfig::ProjectCurve(ladder_config(3.0, None));
    assert!(
        matches!(bare.cut_spec().axial, Axial::SurfaceOffset(Some(_))),
        "with no key the cut keeps the surface offset"
    );
    assert!(bare.cut_spec().axial.depth_after_simulation());
    assert!(
        !bare.set_depth_per_pass(2.0),
        "no feeds door switches the ladder on"
    );
    assert_eq!(bare.depth_per_pass(), None);
    assert!(!bare.has_depth_per_pass());
    // The key is not written to a file that does not state it.
    let text = serde_json::to_string(&ladder_config(3.0, None)).unwrap();
    assert!(!text.contains("depth_per_pass"), "{text}");
}
