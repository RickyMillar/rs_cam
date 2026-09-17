//! P2.d router internals for the unified finishing pass: the greedy
//! band-to-band router and the Shallow band's raster-lattice sampling.
//!
//! Split out of `finish/unified_finish.rs` by P4; every item is unchanged
//! apart from its visibility. The orchestrator that calls them
//! ([`super::unified_finish_toolpath_with_cancel_and_ceiling`]) and the
//! parameter/report types they read stay in the parent.

use crate::finish::finish_planner::FinishBand;
use crate::finish::finish_setup::FinishSurface;
use crate::finish::scallop::ScallopRuntimeAnnotation;
use crate::finish::surface_link::build_surface_link;
use crate::geo::{P2, P3};
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::machine::kinematics::{LinkKinematics, retract_link_time, surface_link_time};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::surface::dropcutter::{
    DropCutterGrid, LatticeSampling, batch_drop_cutter_windowed_with_cancel,
};
#[cfg(test)]
use crate::surface::rest_field::RestGrid;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath};

use super::UnifiedFinishParams;

/// One region's generated toolpath plus the stitch metadata the router
/// needs: where the tool ENTERS the surface (its plunge target), where it
/// LEAVES it (last cut before the trailing retracts), and how many moves
/// each end contributes so a surface link can strip them.
pub(super) struct RegionPath {
    /// Index into the decomposition's `planned.regions`.
    pub(super) region_index: usize,
    pub(super) band: FinishBand,
    pub(super) tp: Toolpath,
    pub(super) anns: Vec<ScallopRuntimeAnnotation>,
    /// Leading moves (Linking rapid + EntryPlunge) removable when a surface
    /// link lands the tool at `entry` directly. 0 = not strippable.
    pub(super) head_strip: usize,
    /// Surface entry point — the first run's EntryPlunge target.
    pub(super) entry: Option<P3>,
    /// Trailing `MoveIntent::Retract` moves removable when the NEXT
    /// junction is a surface link (the tool stays on the surface).
    pub(super) tail_strip: usize,
    /// Surface exit point — the last move before the trailing retracts.
    pub(super) exit: Option<P3>,
}

/// Build the Shallow band's drop-cutter lattice at `direction_deg`.
///
/// Factored out of the band arm by C2 so the shared 0° memo and a
/// per-region PCA-minor lattice are produced by ONE construction. That
/// matters for two reasons beyond tidiness: the off-mesh trench guard and
/// the `stock_to_leave` lift are both load-bearing (see their comments
/// below), and a rotated lattice that skipped either would cut a trench
/// around the part or silently drop the operator's dial on exactly the
/// regions C2 rotates.
///
/// `direction_deg` must already have been through
/// [`crate::geometry::monotone_cells::honest_raster_direction_deg`] when it comes
/// from a measured axis — `batch_drop_cutter_windowed_with_cancel` returns
/// an axis-aligned grid still labelled 90°/180° for those inputs.
///
/// `window` is a world-frame `[x0, y0, x1, y1]` box restricting which of the
/// whole-mesh lattice's points are SAMPLED. The lattice itself — its origin,
/// its phase, its step — is unchanged, so this is not the bbox-trimmed
/// lattice §0j priced at 0.954–0.962×; it is the same lattice with fewer of
/// its points computed. `None` samples the whole mesh (what the shared 0°
/// memo needs, since it serves every region). Build the box with
/// [`region_sampling_window`], which carries the proof that it is a superset
/// of everything the band can emit for that region.
#[allow(clippy::too_many_arguments)] // lattice dials (step, direction, window) ride beside the op params, mirroring the C2 call sites
pub(super) fn build_shallow_raster_grid(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    step_over_mm: f64,
    direction_deg: f64,
    window: Option<[f64; 4]>,
    cancel: &dyn CancelCheck,
) -> Result<DropCutterGrid, Cancelled> {
    // Mesh-bottom floor, mirroring `generate_drop_cutter`'s
    // `effective_min_z` (compute::execute.rs). The stock-bbox floor
    // (`ctx.stock_bbox.min.z - 1.0` there) is the op-adapter's job — this
    // pure-core function only sees a mesh, not a stock model, so callers
    // needing that extra floor should pre-max it into `bottom_z` before
    // calling in.
    let effective_min_z = mesh.bbox.min.z - 0.1;
    // The batch sampler requires `&(dyn CancelCheck + Sync)` for its rayon
    // closures; the orchestrator only receives a plain `&dyn CancelCheck`,
    // matching every finish-op call site up this chain (see the identical
    // constraint documented on
    // `slope::SurfaceHeightmap::from_mesh_with_cancel`). Widening the
    // signature to `+ Sync` would ripple through every future caller for the
    // sake of one internal call, so this builder checks cancellation
    // immediately after the batch call instead of threading `cancel` through
    // it.
    let never_cancel = || false;
    let mut grid = batch_drop_cutter_windowed_with_cancel(
        mesh,
        index,
        cutter,
        &LatticeSampling {
            step_over: step_over_mm,
            direction_deg,
            min_z: effective_min_z,
            window,
        },
        &never_cancel,
    )?;
    // Drop grid points whose vertical ray misses every triangle in the mesh.
    // Replicated from `compute::execute::generate_drop_cutter`'s identical
    // guard: `point_drop_cutter` marks a point contacted whenever the cutter
    // (which has radius) touches ANY nearby triangle — including the rim of
    // a mesh that doesn't cover that XY. Without this check the tool rides
    // the edge and carves a trench around the part.
    for pt in &mut grid.points {
        let mut over = false;
        for &tri_idx in &index.query(pt.x, pt.y, 0.0) {
            // SAFETY: tri_idx comes from `index.query`, which only ever
            // returns indices into `mesh.faces` (mirrors
            // `generate_drop_cutter`'s identical loop in compute::execute.rs).
            #[allow(clippy::indexing_slicing)]
            let tri = &mesh.faces[tri_idx];
            if tri.contains_point_xy(pt.x, pt.y) {
                over = true;
                break;
            }
        }
        if !over {
            pt.z = effective_min_z;
            pt.contacted = false;
        }
    }
    // D-16.2 (F3, 2026-08-06): honour `stock_to_leave`.
    //
    // `raster_toolpath_from_grid` emits every target as
    // `grid.get(row, col).position()` verbatim — it takes no stock-to-leave
    // argument and does no Z arithmetic — so until this lift the dial was
    // silently dropped on the whole Shallow band while the MidSteep band
    // next door honoured it. The convention is the repo's shared one: a pure
    // **+Z shift on the drop-cutter contact point**, identical to
    // `scallop.rs`'s `cl.z + stock_to_leave` and to `steep_shallow.rs`'s
    // shipped shallow raster. (It is an approximation — what survives
    // measured normal to the surface is `stock_to_leave·cos θ` — but it is
    // the SAME approximation every finish op in the repo makes, and
    // diverging here alone would put a step at every band seam.)
    //
    // The off-mesh sentinel moves WITH the grid, and the filter threshold at
    // the call site moves with it too. Lifting the sentinel while leaving
    // the threshold at `effective_min_z` would stop off-mesh points being
    // filtered and let the tool ride the mesh rim — the exact trench the
    // coverage guard above prevents.
    if params.stock_to_leave != 0.0 {
        for pt in &mut grid.points {
            pt.z += params.stock_to_leave;
        }
    }
    check_cancel(cancel)?;
    Ok(grid)
}

/// Slope (deg) below which the Shallow honest-raster derate does not run.
///
/// `sec(1°) - 1 = 0.00015` — three orders of magnitude under the 2%
/// acceptance tolerance the Track B instrument uses (`CLEAN` at
/// `<= 1.02 x s_max`). Skipping the derate under this floor keeps a flat
/// region on the shared 0° memo, byte-identical to the pre-fix emission,
/// and avoids one private lattice build per effectively-flat region.
pub(super) const SHALLOW_DERATE_MIN_SLOPE_DEG: f64 = 1.0;

/// The maximum slope (deg) one Shallow region's covered cells carry, read
/// from the classification slope map — `theta_max` for the honest-raster
/// derate (Track B fix).
///
/// Two guards:
///
/// * A cell counts only when it AND its in-grid 4-neighbours are
///   geometrically covered. An uncovered cell carries the `min_z`
///   bbox-floor clamp in the Z grid (see [`crate::surface::slope::GridZ`]), so the
///   finite differences beside a coverage edge read a cliff the mesh does
///   not have; such a cliff must not set the derate.
/// * The result is clamped to `clamp_deg` (the planner's
///   `steep_threshold_deg`). The seam dilation (`overlap_mm`) can pull a
///   fringe of steeper cells into a Shallow polygon; those cells belong to
///   the neighbouring band's mechanism, and one near-vertical fringe cell
///   would collapse the stepover.
///
/// A region with no qualifying cell returns `0.0` — no derate. Such a
/// region is all rim; it has no measured slope to derate against.
pub(super) fn shallow_region_max_slope_deg(
    surface: &FinishSurface,
    covered: &[bool],
    regions: &RegionSet<'_>,
    clamp_deg: f64,
) -> f64 {
    let cols = surface.cols();
    if cols == 0 {
        return 0.0;
    }
    let rows = surface.slope_map.rows;
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;
    let geom = surface.heightmap.covered_flags();
    let geom_at =
        |row: usize, col: usize| -> bool { geom.get(row * cols + col).copied().unwrap_or(false) };

    let mut max_rad = 0.0_f64;
    for (i, &angle) in surface.slope_map.angles.iter().enumerate() {
        if !covered.get(i).copied().unwrap_or(false) {
            continue;
        }
        let row = i / cols;
        let col = i % cols;
        // In-grid 4-neighbours must be geometrically covered, or this
        // cell's gradient read the coverage cliff.
        let neighbours_ok = geom_at(row, col)
            && (row == 0 || geom_at(row - 1, col))
            && (row + 1 >= rows || geom_at(row + 1, col))
            && (col == 0 || geom_at(row, col - 1))
            && (col + 1 >= cols || geom_at(row, col + 1));
        if !neighbours_ok {
            continue;
        }
        let x = origin_x + col as f64 * cell;
        let y = origin_y + row as f64 * cell;
        if regions.contains(&P2::new(x, y)) {
            max_rad = max_rad.max(angle);
        }
    }
    max_rad.to_degrees().min(clamp_deg)
}

/// The world-frame box of whole-mesh lattice points ONE Shallow region can
/// ever emit — the sampling window for its private PCA-minor lattice.
///
/// # Why a bbox pad is enough, and why exactly one stepover
///
/// Two filters stand between the lattice and an emitted move, and both are
/// strict containment tests (`RegionSet::contains` → `Polygon2::
/// contains_point`, no tolerance):
///
/// * the **undivided** arm filters on the region polygon itself, so every
///   point it can emit is inside `polygon.bbox()`;
/// * the **cell** arm filters on a cell polygon that
///   `monotone_cells::polygons_for_lattice_cell` reconstructs by marching
///   squares over lattice points that are themselves inside the region. That
///   contour runs at the mid-point between a selected point and its
///   unselected neighbour — `0.5 · stepover` outside the outermost selected
///   point — and the next lattice point out is a full step away, so no
///   lattice point outside the region bbox can fall inside a cell.
///
/// One stepover of pad therefore covers the cell arm's reach with a
/// half-step to spare, and the window is a strict superset of both
/// populations. It is **not** an offset of the region: moving the lattice is
/// the thing this whole design refuses to do.
///
/// There is deliberately no extra "conditioning margin" term. The shallow
/// band's region polygons carry none of their own — `region_margin_mm` is
/// the REST-mask dilation, applied on the rest grid before `decompose`, so
/// whatever it added is already inside `polygon`.
pub(super) fn region_sampling_window(polygon: &Polygon2, raster_stepover: f64) -> [f64; 4] {
    let [x0, y0, x1, y1] = polygon.bbox();
    // A non-finite stepover makes the whole window non-finite, and
    // `dropcutter::lattice_index_window` answers that with the UNWINDOWED
    // lattice — the correct answer, just the expensive one.
    let pad = raster_stepover.abs();
    [x0 - pad, y0 - pad, x1 + pad, y1 + pad]
}

/// A costed junction between two consecutive regions in the route.
pub(super) struct JunctionChoice {
    pub(super) surface: bool,
    /// Interior surface-link points (empty for a retract junction, and for
    /// a zero-length surface junction).
    pub(super) pts: Vec<P3>,
    pub(super) cost_s: f64,
    pub(super) alt_cost_s: Option<f64>,
}

pub(super) fn band_rank(band: FinishBand) -> u8 {
    match band {
        FinishBand::VerySteep => 2,
        FinishBand::MidSteep => 1,
        FinishBand::Shallow => 0,
    }
}

/// Leading strippable preamble: the index `k` of the first
/// `FinishingCut` move plus the surface entry point (the move `k-1`
/// EntryPlunge target). `None` when the toolpath doesn't open with the
/// canonical `Linking rapid(s) → EntryPlunge → FinishingCut` shape every
/// strategy generator in this module emits — an unrecognized preamble is
/// kept verbatim rather than guessed at.
pub(super) fn strippable_preamble(tp: &Toolpath) -> Option<(usize, P3)> {
    let k = tp
        .moves
        .iter()
        .position(|m| m.intent == MoveIntent::FinishingCut)?;
    if k == 0 {
        return None;
    }
    let head_ok = tp
        .moves
        .iter()
        .take(k)
        .all(|m| matches!(m.intent, MoveIntent::Linking | MoveIntent::EntryPlunge));
    if !head_ok {
        return None;
    }
    let plunge = tp.moves.get(k - 1)?;
    (plunge.intent == MoveIntent::EntryPlunge).then_some((k, plunge.target))
}

/// Trailing retract count + the surface exit point right before them.
pub(super) fn trailing_retracts(tp: &Toolpath) -> (usize, Option<P3>) {
    let n = tp
        .moves
        .iter()
        .rev()
        .take_while(|m| m.intent == MoveIntent::Retract)
        .count();
    let exit = tp
        .moves
        .len()
        .checked_sub(n + 1)
        .and_then(|i| tp.moves.get(i))
        .map(|m| m.target);
    (n, exit)
}

/// Greedy nearest-by-integrated-link-time route over the generated
/// regions, seeded at the steepest band present (first planned region of
/// that band — deterministic). Returns the order (indices into `paths`)
/// and the winning junction choice for each consecutive pair. 2-opt is
/// deliberately absent: the design doc's P0 discipline is measure first —
/// it only gets built if the A/B says greedy leaves >5% on the table.
#[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
pub(super) fn route_greedy(
    paths: &[RegionPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    machining_boundary: Option<&RegionSet<'_>>,
    lk: &LinkKinematics,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<usize>, Vec<Option<JunctionChoice>>), Cancelled> {
    let seed = (0..paths.len()).max_by_key(|&i| {
        (
            paths.get(i).map_or(0, |rp| band_rank(rp.band)),
            std::cmp::Reverse(i),
        )
    });
    let Some(seed) = seed else {
        return Ok((Vec::new(), Vec::new()));
    };

    let mut visited = vec![false; paths.len()];
    if let Some(v) = visited.get_mut(seed) {
        *v = true;
    }
    let mut order = vec![seed];
    let mut junctions: Vec<Option<JunctionChoice>> = Vec::new();
    let mut current = seed;

    for _ in 1..paths.len() {
        check_cancel(cancel)?;
        let from = paths
            .get(current)
            .and_then(|rp| rp.exit.or_else(|| rp.tp.moves.last().map(|m| m.target)));
        let Some(from) = from else { break };

        let mut best: Option<(usize, JunctionChoice)> = None;
        for (j, candidate) in paths.iter().enumerate() {
            if visited.get(j).copied().unwrap_or(true) {
                continue;
            }
            let choice = choose_link(
                from,
                candidate,
                mesh,
                index,
                cutter,
                params,
                machining_boundary,
                lk,
            );
            // Strict `<` keeps the earliest candidate on exact ties —
            // deterministic (`paths` preserves the planned-region order).
            if best.as_ref().is_none_or(|(_, b)| choice.cost_s < b.cost_s) {
                best = Some((j, choice));
            }
        }
        let Some((next, choice)) = best else { break };
        if let Some(v) = visited.get_mut(next) {
            *v = true;
        }
        order.push(next);
        junctions.push(Some(choice));
        current = next;
    }

    Ok((order, junctions))
}

/// Cost both link candidates from `from` (the previous region's surface
/// exit) into `to` and return the winner. The surface candidate mirrors
/// pencil's emit decision exactly: gouge-checked via [`build_surface_link`]
/// (real cutter → it rides the OFFSET surface), additionally required to
/// stay inside the machining boundary (the selective-scallop gouge rule:
/// never feed across excluded islands), and only available when the
/// follower has a strippable entry. Ties go to the surface link, matching
/// pencil's `surface_t <= retract_t`.
#[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
fn choose_link(
    from: P3,
    to: &RegionPath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    machining_boundary: Option<&RegionSet<'_>>,
    lk: &LinkKinematics,
) -> JunctionChoice {
    let entry_surface = (to.head_strip > 0).then_some(to.entry).flatten();
    let retract_to = entry_surface
        .or(to.entry)
        .or_else(|| to.tp.moves.first().map(|m| m.target))
        .unwrap_or(from);
    let retract_t = retract_link_time(
        from,
        retract_to,
        params.safe_z,
        // No rapid-down-to-clearance: the cost model can't verify the input
        // stock's ceiling from here (that check lives in the post-generation
        // `optimize_entry_descents` pass), so it must not assume a descent
        // it can't guarantee is safe — same reasoning as pencil's emit path.
        None,
        params.plunge_rate,
        &lk.kinematics,
        lk.max_feed_mm_min,
        lk.rapid_feed_mm_min,
    );

    let surface_candidate = entry_surface.and_then(|entry| {
        let pts = build_surface_link(
            from,
            entry,
            mesh,
            index,
            cutter,
            params.stock_to_leave,
            params.sampling,
        )?;
        // Inside the machining boundary only — a surface feed across an
        // excluded island is exactly the gouge class the boundary exists
        // to prevent.
        if let Some(boundary) = machining_boundary
            && !pts.iter().all(|p| boundary.contains(&P2::new(p.x, p.y)))
        {
            return None;
        }
        let mut costed_path = pts.clone();
        costed_path.push(entry);
        let surface_t = surface_link_time(
            from,
            &costed_path,
            params.feed_rate,
            &lk.kinematics,
            lk.max_feed_mm_min,
            lk.rapid_feed_mm_min,
        );
        Some((pts, surface_t))
    });

    match surface_candidate {
        Some((pts, surface_t)) if surface_t <= retract_t => JunctionChoice {
            surface: true,
            pts,
            cost_s: surface_t,
            alt_cost_s: Some(retract_t),
        },
        Some((_, surface_t)) => JunctionChoice {
            surface: false,
            pts: Vec::new(),
            cost_s: retract_t,
            alt_cost_s: Some(surface_t),
        },
        None => JunctionChoice {
            surface: false,
            pts: Vec::new(),
            cost_s: retract_t,
            alt_cost_s: None,
        },
    }
}

/// Z range (min, max) of the classification surface's covered cells whose
/// centers fall inside `regions`. `None` when nothing survives both
/// filters — a genuinely degenerate band (its own polygon shrank to
/// nothing after conditioning, or every covered cell sits just outside it
/// at the mask boundary).
pub(super) fn band_z_range(
    surface: &FinishSurface,
    covered: &[bool],
    regions: &RegionSet<'_>,
) -> Option<(f64, f64)> {
    let cols = surface.cols();
    if cols == 0 {
        return None;
    }
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;

    let mut min_z = f64::INFINITY;
    let mut max_z = f64::NEG_INFINITY;
    for (i, &z) in surface
        .heightmap
        .z_or_bbox_floor_values()
        .iter()
        .enumerate()
    {
        if !covered.get(i).copied().unwrap_or(false) {
            continue;
        }
        let row = i / cols;
        let col = i % cols;
        let x = origin_x + col as f64 * cell;
        let y = origin_y + row as f64 * cell;
        if regions.contains(&P2::new(x, y)) {
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    }
    (min_z.is_finite() && max_z.is_finite()).then_some((min_z, max_z))
}

// ── Claims pipeline internals (v3 S1/S4) ────────────────────────────────

/// Nearest-cell index into a [`RestGrid`] at world `(x, y)`. The rest
/// grid's own origin/cell size (from [`detect_rest_valleys`]) is
/// independent of the classification surface's grid, so this is
/// deliberately a round-to-nearest lookup rather than an index computed
/// from the caller's grid geometry — mirroring [`RestReference::Stock`]'s
/// nearest-cell-only contract (never interpolate a dexel top across a
/// steep wall). `None` outside the grid.
///
/// Test-only since the S2 removal (2026-08-03): the S4 mask walks the
/// rest grid in its own index space and needs no world-XY lookup, but the
/// claims sentries probe specific world points to prove which reference
/// arm fed the detector.
#[cfg(test)]
pub(super) fn rest_grid_index(grid: &RestGrid, x: f64, y: f64) -> Option<usize> {
    let spec = grid.grid;
    if spec.nx == 0 || spec.ny == 0 || spec.cell_mm <= 0.0 {
        return None;
    }
    let col = ((x - spec.origin_x) / spec.cell_mm).round();
    let row = ((y - spec.origin_y) / spec.cell_mm).round();
    if col < 0.0 || row < 0.0 {
        return None;
    }
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let (col, row) = (col as usize, row as usize);
    if col >= spec.nx || row >= spec.ny {
        return None;
    }
    Some(spec.index_of(row, col))
}

/// Total length (mm) of `tp`'s `FinishingCut` moves — each move's
/// contribution is the distance from the PRECEDING move's target (rapid,
/// plunge, or another cut) to its own, so a cut immediately following a
/// plunge/link still counts the segment actually cut. Used for the crease
/// node's `crease_path_length_mm` telemetry instead of reaching into
/// `pencil::PencilPath`'s private fields.
pub(super) fn cutting_length_mm(tp: &Toolpath) -> f64 {
    let mut total = 0.0;
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        if mv.intent == MoveIntent::FinishingCut
            && let Some(p) = prev
        {
            total += ((mv.target.x - p.x).powi(2)
                + (mv.target.y - p.y).powi(2)
                + (mv.target.z - p.z).powi(2))
            .sqrt();
        }
        prev = Some(mv.target);
    }
    total
}
