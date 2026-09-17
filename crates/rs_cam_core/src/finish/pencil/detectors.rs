//! The three detector arms and the reference-tool resolution they share.
//!
//! Each arm turns one valley-detection front-end (curvature crest lines, the
//! rest-depth field, mesh dihedral creases) into `PencilPath`s through the
//! shared `chain_paths::paths_from_sampled` pipeline. Split out of
//! `finish/pencil.rs` (P4). The dispatcher that picks an arm stays in the
//! parent.

use std::collections::HashMap;

use tracing::{info, warn};

use crate::compute::config::TipFloatFinding;
use crate::finish::pencil_dihedral::{
    EdgeKey, SharedEdge, build_edge_adjacency, chain_concave_edges, compute_shared_edges,
    sample_chain_bisected,
};
use crate::geo::{P3, V3, polyline_length, resample_polyline};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::trace::debug_trace::ToolpathDebugContext;

use super::chain_paths::{OffsetFan, paths_from_sampled};
use super::emission::tip_contact_radius;
use super::{PencilParams, PencilPath, gate_chains_by_depth, polyline_passes_depth};

/// Diameter (mm) of the vanishingly small "surface probe" ball substituted
/// when there's no real or nominal reference tool to compare against (the
/// self-referenced-gap case, [`ResolvedReference::SelfReferenced`] /
/// [`crate::surface::rest_field::RestReference`]'s `is_surface_probe` arm). Small
/// enough to hug the raw surface without perturbing the rest-depth
/// comparison. Shared with `rest_field`'s test hillshade harness, which
/// probes the same way to render a DEM.
pub(crate) const SURFACE_PROBE_BALL_DIAMETER_MM: f64 = 0.1;
/// Overall tool length (mm) paired with [`SURFACE_PROBE_BALL_DIAMETER_MM`].
/// Irrelevant to the rest-depth math (only the ball tip matters) — picked to
/// be a plausible tool length rather than for any geometric reason.
pub(crate) const SURFACE_PROBE_BALL_LENGTH_MM: f64 = 10.0;
/// Overall tool length (mm) for a synthesized nominal reference ball
/// ([`ResolvedReference::Nominal`]) when no real reference-tool geometry is
/// set. Like the probe length above, irrelevant to the ball-tip rest height
/// this synthesizes — just needs to be a plausible tool length.
pub(crate) const NOMINAL_REFERENCE_BALL_LENGTH_MM: f64 = 25.0;

/// Result of resolving the rest-depth reference tool (R1), shared by all
/// three detectors and by the `pencil_chain_count` test helper — this is the
/// ONE place the "real tool overrides a nominal ball" priority is encoded,
/// replacing three separate copies (the Dihedral/Curvature gate call, the
/// RestDepth field call, and the test helper). Priority: a real library tool
/// (`params.reference_cutter`, true geometry) → a nominal ball at
/// `reference_tool_diameter` when bigger than the pencil tool → no reference
/// at all (self-referenced gap). Owns the synthesized nominal ball so its
/// borrow outlives the call.
pub(super) enum ResolvedReference<'a> {
    /// A real library tool's true cutter geometry (`params.reference_cutter`).
    Real(&'a dyn MillingCutter),
    /// A nominal ball at `reference_tool_diameter`, synthesized because no
    /// real reference tool was set and the nominal diameter exceeds the
    /// pencil tool's own. Length is irrelevant to the ball-tip rest height.
    Nominal(crate::tool::BallEndmill),
    /// No reference tool: the Dihedral/Curvature gate treats this as a
    /// self-referenced gap; the RestDepth field substitutes a tiny
    /// bare-surface probe with the sign flipped (see [`RestReference`]).
    ///
    /// [`RestReference`]: crate::surface::rest_field::RestReference
    SelfReferenced,
}

impl ResolvedReference<'_> {
    /// The `Option<&dyn MillingCutter>` shape the Dihedral/Curvature gates
    /// want: `None` means self-referenced gap.
    pub(super) fn as_dyn(&self) -> Option<&dyn MillingCutter> {
        match self {
            ResolvedReference::Real(r) => Some(*r),
            ResolvedReference::Nominal(b) => Some(b),
            ResolvedReference::SelfReferenced => None,
        }
    }
}

/// Resolve the rest-depth reference tool (see [`ResolvedReference`]).
///
/// # The scale the comparison is against (task #12, H2.1)
///
/// "Is the nominal reference bigger than the pencil tool?" is a question
/// about the scale the pencil actually CUTS with — its tip sphere — not
/// about the widest point of its body. It used to be asked against
/// `cutter.diameter()`, which for a tapered ball deliberately reports the
/// SHANK (`TOOL_SCALE_SEMANTICS.md` §4.1). On the shipped Ø1-tip / Ø6-shank
/// taper that made the comparison `6.0 > 6.0 + 1e-6` — false at the shipped
/// `reference_tool_diameter` default of 6.0 — so **every tapered pencil
/// operation silently fell through to the self-referenced surface probe**,
/// and no rest measurement on a tapered tool meant what it said. Confirmed
/// live before the fix by `CHECKPOINT_A_EVIDENCE.md` probe 3: 1 chain at the
/// default reference versus 2 at a reference above the shank, same tool and
/// same fixture.
///
/// The comparison is now against `valley_radius_mm() * 2` — the diameter that
/// has to FIT, which is `diameter()` for every non-tapered shape, so nothing
/// but the tapered path moves.
///
/// G-BULLCUSP (2026-09-10) moved this from `cusp_radius_mm()` to
/// `valley_radius_mm()`. The two returned the same number for every shape
/// until a bull nose separated them, and this site asks the FIT question: a
/// reference tool is "finer than the pencil" only if it can get where the
/// pencil can, and a bull nose cannot do that on its corner radius. The
/// change is byte-identical for every tool.
pub(super) fn resolve_reference_cutter<'a>(
    params: &'a PencilParams,
    pencil: &dyn MillingCutter,
) -> ResolvedReference<'a> {
    if let Some(rc) = params.reference_cutter.as_ref() {
        return ResolvedReference::Real(rc);
    }
    let pencil_cutting_diameter = pencil.valley_radius_mm() * 2.0;
    if params.reference_tool_diameter > pencil_cutting_diameter + 1e-6 {
        return ResolvedReference::Nominal(crate::tool::BallEndmill::new(
            params.reference_tool_diameter,
            NOMINAL_REFERENCE_BALL_LENGTH_MM,
        ));
    }
    ResolvedReference::SelfReferenced
}

/// `Curvature` detector arm (see [`crate::finish::crest_lines`]): trace the zero-set
/// of the minimal-curvature extremality, filtered by the single
/// `valley_saliency` (|κ₂|) dial. Keep lines long enough, then optionally
/// apply the reference-tool rest-depth gate (only when `min_valley_depth >
/// 0`, so saliency alone can drive selection), resample to cut spacing, and
/// build paths. No bisector — the crest line already sits on the valley
/// floor. Polls `cancel` once per line, matching the other two arms.
pub(super) fn curvature_arm(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    float: &mut TipFloatFinding,
    cancel: &dyn CancelCheck,
) -> Result<Vec<PencilPath>, Cancelled> {
    let mut all_paths: Vec<PencilPath> = Vec::new();
    let gap_threshold = params.min_valley_depth;
    let resolved = resolve_reference_cutter(params, cutter);
    let reference_ref = resolved.as_dyn();

    let cp = crate::finish::crest_lines::CrestParams {
        valley_saliency: params.valley_saliency,
        smoothing_iters: params.curvature_smoothing,
        min_line_length: params.min_cut_length,
    };
    let lines = crate::finish::crest_lines::detect_valley_lines(mesh, &cp);
    let apply_rest_gate = gap_threshold > 0.0;
    let kept: Vec<Vec<P3>> = lines
        .into_iter()
        .filter(|l| polyline_length(l) >= params.min_cut_length)
        .filter(|l| {
            !apply_rest_gate
                || polyline_passes_depth(l, mesh, index, cutter, reference_ref, gap_threshold)
        })
        .collect();
    if kept.is_empty() {
        info!("Curvature detector: no valley lines passed length/rest-depth gate");
        return Ok(all_paths);
    }
    let chain_total = kept.len();
    info!(
        valley_lines = chain_total,
        gap_threshold,
        valley_saliency = params.valley_saliency,
        "Curvature detector: valley crest lines built"
    );
    for (ci, line) in kept.iter().enumerate() {
        check_cancel(cancel)?;
        let sampled = resample_polyline(line, params.sampling);
        paths_from_sampled(
            &sampled,
            ci + 1,
            chain_total,
            mesh,
            index,
            cutter,
            params.stock_to_leave,
            params.offset_stepover,
            OffsetFan::symmetric(params.num_offset_passes),
            &mut all_paths,
            float,
        );
    }
    Ok(all_paths)
}

/// `RestDepth` detector arm (see [`crate::surface::rest_field`]): build the dual-tool
/// rest field, threshold at `min_valley_depth`, thin each region to a
/// skeleton, and route by width. The per-polyline [`polyline_passes_depth`]
/// gate is REDUNDANT here (the field threshold IS that same quantity,
/// pointwise over the whole grid) — skip it.
///
/// Rest-field reference. R2: prefer the ACTUAL machined stock the prior
/// toolpaths left (`initial_stock`, when this op cuts FromRemainingStock and
/// a prior sim exists) — it captures the real prior toolpath pattern
/// (scallop cusps, skipped boundaries, walls the finish never visited), not
/// just a tool's shape. Frame guard (the F-024 lesson): the stock z_grid and
/// the mesh the pencil drops against must share a frame; require their XY
/// bboxes to overlap or a silent frame mismatch would read garbage rest
/// everywhere. On no stock / non-overlap, fall back to the R1 cutter
/// resolution ([`resolve_reference_cutter`]): real reference tool or the
/// bigger nominal ball (`is_surface_probe = false`), else a tiny
/// bare-surface probe with the sign flipped. NOTE the sign-flip trap: a
/// *real* reference equal to the pencil tool still yields rest ≈ 0 via
/// `ref_z − pencil_z`, NOT probe mode.
#[allow(clippy::too_many_arguments)]
pub(super) fn rest_depth_arm(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    debug: Option<&ToolpathDebugContext>,
    out: &mut super::PencilReport,
    float: &mut TipFloatFinding,
    cancel: &dyn CancelCheck,
) -> Result<Vec<PencilPath>, Cancelled> {
    let stock_ref = initial_stock.filter(|stock| {
        let (sb, mb) = (&stock.stock_bbox, &mesh.bbox);
        let overlap = sb.min.x <= mb.max.x
            && sb.max.x >= mb.min.x
            && sb.min.y <= mb.max.y
            && sb.max.y >= mb.min.y;
        if !overlap {
            warn!(
                stock = format!(
                    "[{:.1},{:.1}]..[{:.1},{:.1}]",
                    sb.min.x, sb.min.y, sb.max.x, sb.max.y
                ),
                mesh = format!(
                    "[{:.1},{:.1}]..[{:.1},{:.1}]",
                    mb.min.x, mb.min.y, mb.max.x, mb.max.y
                ),
                "Rest-depth detector: stock/mesh XY frames do not overlap; \
                 falling back to tool reference"
            );
        }
        overlap
    });
    let probe_ball =
        crate::tool::BallEndmill::new(SURFACE_PROBE_BALL_DIAMETER_MM, SURFACE_PROBE_BALL_LENGTH_MM);
    let resolved = resolve_reference_cutter(params, cutter);
    // reference-mode counter: 0 = nominal ball / probe, 1 = real tool,
    // 2 = machined stock.
    let (reference, probe_mode, rest_reference_mode): (
        crate::surface::rest_field::RestReference<'_>,
        bool,
        u8,
    ) = if let Some(stock) = stock_ref {
        (
            crate::surface::rest_field::RestReference::Stock(stock),
            false,
            2,
        )
    } else {
        match &resolved {
            ResolvedReference::Real(r) => (
                crate::surface::rest_field::RestReference::Cutter {
                    tool: *r,
                    is_surface_probe: false,
                },
                false,
                1,
            ),
            ResolvedReference::Nominal(b) => (
                crate::surface::rest_field::RestReference::Cutter {
                    tool: b as &dyn MillingCutter,
                    is_surface_probe: false,
                },
                false,
                0,
            ),
            ResolvedReference::SelfReferenced => (
                crate::surface::rest_field::RestReference::Cutter {
                    tool: &probe_ball as &dyn MillingCutter,
                    is_surface_probe: true,
                },
                true,
                0,
            ),
        }
    };
    let rf_params = crate::surface::rest_field::RestFieldParams {
        cell_mm: params.rest_cell_mm,
        min_valley_depth: params.min_valley_depth,
        // Coverage routing (PR-5): the detector routes against the fan this
        // arm is about to emit, so it is handed the SAME two numbers
        // `centerline_cut_paths` gets below. The old `route_width_factor`
        // threshold is deleted (FIN-04).
        offset_stepover_mm: params.offset_stepover,
        num_offset_passes_cap: params.num_offset_passes,
        min_cut_length: params.min_cut_length,
        // No dedicated PencilParams dial yet — the default margin (P2.1
        // scope: derive region_polygons, not expose a new user-facing knob).
        ..Default::default()
    };
    let rf =
        crate::surface::rest_field::detect_rest_valleys(mesh, index, cutter, reference, &rf_params);
    let report = &rf.report;
    info!(
        rest_volume_mm3 = format!("{:.1}", report.total_rest_volume_mm3),
        pencil_regions = report.pencil_region_count,
        clearing_regions = report.clearing_region_count,
        centerlines = rf.centerlines.len(),
        coverage = format!("{:.2}", report.coverage()),
        probe_mode,
        rest_reference_mode,
        "Rest-depth detector: rest field built (mode 0=nominal 1=tool 2=stock)"
    );
    if let Some(dbg) = debug {
        let scope = dbg.start_span("rest_field", "rest-depth report");
        scope.set_counter("rest_volume_mm3", report.total_rest_volume_mm3);
        scope.set_counter("rest_pencil_regions", report.pencil_region_count as f64);
        scope.set_counter("rest_clearing_regions", report.clearing_region_count as f64);
        scope.set_counter("rest_skeleton_mm", report.skeleton_length_mm);
        scope.set_counter("rest_traced_mm", report.traced_length_mm);
        scope.set_counter("rest_coverage", report.coverage());
        scope.set_counter("rest_reference_mode", rest_reference_mode as f64);
        scope.finish();
    }
    for reg in &rf.clearing_regions {
        info!(
            bbox = format!(
                "[{:.1},{:.1}]..[{:.1},{:.1}]",
                reg.bbox[0], reg.bbox[1], reg.bbox[2], reg.bbox[3]
            ),
            cells = reg.cell_count,
            peak_rest_mm = format!("{:.2}", reg.peak_rest_mm),
            "Rest-depth detector: wide region routed to clearing (Phase D: adaptive3d)"
        );
    }
    // Hand the rest-field grid to the caller for the GUI heatmap overlay
    // (set even when no centreline survives the length gate below).
    out.rest_grid = Some(rf.rest_grid);
    // Same for the derived machining-region polygons (P2.2 selective-finishing
    // boundary source) — set alongside the grid, independent of whether any
    // centreline survives the length gate.
    out.rest_regions = Some(rf.region_polygons);
    // Length-gate + resample + width-capped offset-pass emission, factored
    // into `crease_paths::centerline_cut_paths` so the P2 finish planner's
    // future crease pass can reuse it without pencil's detector dispatch.
    let all_paths = crate::finish::crease_paths::centerline_cut_paths(
        &rf.centerlines,
        mesh,
        index,
        cutter,
        params.sampling,
        params.offset_stepover,
        params.num_offset_passes,
        params.min_cut_length,
        params.stock_to_leave,
        float,
        cancel,
    )?;
    if all_paths.is_empty() {
        info!("Rest-depth detector: no centerlines passed length gate");
    }
    Ok(all_paths)
}

/// `Dihedral` detector arm (see [`crate::finish::pencil_dihedral`]): the historical
/// mesh-crease detector. Angle-filter candidate edges, chain them, keep only
/// chains holding genuine REST material (`rest_depth = reference_gap −
/// pencil_gap > threshold`), then sample each surviving chain
/// bisector-positioned into cut-spaced paths.
pub(super) fn dihedral_arm(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    float: &mut TipFloatFinding,
    cancel: &dyn CancelCheck,
) -> Result<Vec<PencilPath>, Cancelled> {
    let mut all_paths: Vec<PencilPath> = Vec::new();
    let gap_threshold = params.min_valley_depth;
    let resolved = resolve_reference_cutter(params, cutter);
    let reference_ref = resolved.as_dyn();

    // Step 1: edge adjacency. Step 2: shared edges + dihedral angles.
    let edge_map = build_edge_adjacency(mesh);
    let shared_edges = compute_shared_edges(mesh, &edge_map);

    // Step 3: concave-edge candidate set (angle filter only; the real
    // selection is the per-chain rest-depth gate). The angle filter alone
    // floods dense organic meshes — every triangulation crease passes.
    let threshold_rad = params.bitangency_angle.to_radians();
    let total_shared = shared_edges.len();
    let concave_owned: Vec<SharedEdge> = shared_edges
        .into_iter()
        .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
        .collect();
    if concave_owned.is_empty() {
        info!(
            "No concave edges under {:.0}° threshold",
            params.bitangency_angle
        );
        return Ok(all_paths);
    }
    check_cancel(cancel)?;

    // Step 4: chain concave edges, then keep only chains holding genuine
    // REST material (rest_depth = reference_gap − pencil_gap > threshold).
    let chains_all = chain_concave_edges(&concave_owned, mesh, params.min_cut_length);
    let chains = gate_chains_by_depth(
        chains_all,
        mesh,
        index,
        cutter,
        reference_ref,
        gap_threshold,
    );

    // Per-edge wall normals for bisector positioning; built from the
    // filtered concave edges so the offset uses the exact two walls.
    #[allow(clippy::indexing_slicing)] // face_a/face_b are valid mesh face indices
    let edge_norms: HashMap<EdgeKey, (V3, V3)> = concave_owned
        .iter()
        .map(|e| {
            (
                e.key,
                (mesh.faces[e.face_a].normal, mesh.faces[e.face_b].normal),
            )
        })
        .collect();
    // Tool contact radius: the ball/corner radius that nestles into the
    // corner. `corner_radius_mm()` is only overridden by flat and bullnose
    // cutters, so ball-family tools fall through — and for a TAPERED ball
    // that fallback used to land on `radius()`, the SHAFT radius (3.0 mm for
    // a Ø1 tip). The bisector then positioned as if a 3 mm ball nestled into
    // the corner, six times wider than the tip that actually touches it.
    // `cusp_radius_mm()` is the tip sphere for tapered balls and identical
    // to `radius()` for every other cutter, so this is a no-op off the taper.
    // One definition, shared with the entry-ramp planner.
    let contact_radius = tip_contact_radius(cutter);

    if chains.is_empty() {
        info!(
            "No tool-unreachable valley chains (gap > {:.3}mm) over {:.1}mm",
            gap_threshold, params.min_cut_length
        );
        return Ok(all_paths);
    }
    info!(
        total_shared,
        chains = chains.len(),
        gap_threshold,
        "Pencil detection complete (chain-level reach-gap gate)"
    );

    // Step 5: sample each chain (bisector-positioned) → paths.
    let chain_total = chains.len();
    for (chain_index, chain) in chains.iter().enumerate() {
        check_cancel(cancel)?;
        let sampled = sample_chain_bisected(
            mesh,
            chain,
            params.sampling,
            &edge_norms,
            contact_radius,
            params.bisector_strength,
        );
        paths_from_sampled(
            &sampled,
            chain_index + 1,
            chain_total,
            mesh,
            index,
            cutter,
            params.stock_to_leave,
            params.offset_stepover,
            OffsetFan::symmetric(params.num_offset_passes),
            &mut all_paths,
            float,
        );
    }
    Ok(all_paths)
}
