//! Pencil finishing — traces concave edges (creases) on mesh surfaces.
//!
//! Owns orchestration (reference-tool resolution, detector dispatch) and the
//! shared pipeline every detector arm feeds into: fair → lift-to-surface →
//! rest-depth gate → offset passes → nearest-neighbor order → emit. The
//! three valley-detection front-ends themselves ([`PencilDetector`]) each
//! live in their own module, mirrored on the [`crate::finish::crest_lines`] /
//! [`crate::surface::rest_field`] pattern:
//! - **Dihedral** (the historical default) — [`crate::finish::pencil_dihedral`]:
//!   mesh-crease detection via per-edge dihedral angle + graph chaining.
//! - **Curvature** — [`crate::finish::crest_lines`]: curvature crest-line extraction,
//!   the right choice for dense noisy organic relief.
//! - **RestDepth** — [`crate::surface::rest_field`]: the tool-radius-aware dual-tool
//!   rest field.
//!
//! Each arm ([`dihedral_arm`], [`curvature_arm`], [`rest_depth_arm`]) turns
//! its detector's raw output into `PencilPath`s via the shared
//! [`paths_from_sampled`]; [`pencil_toolpath_structured_annotated_with_cancel`]
//! dispatches to the right arm, then orders and emits.
//!
//! The step machinery lives in three private children: `chain_paths`
//! (fair / lift / offset / gate), `emission` (entry ramp, link lift, emit)
//! and `detectors` (the three arms and reference-tool resolution).

mod chain_paths;
mod detectors;
mod emission;

pub(crate) use chain_paths::{OffsetFan, paths_from_sampled, reach_gap_threshold};
/// The only reader outside this module is `surface::rest_field`'s test
/// module, so the re-export is `cfg(test)` to keep the lib build warning-free.
#[cfg(test)]
pub(crate) use detectors::NOMINAL_REFERENCE_BALL_LENGTH_MM;
pub(crate) use detectors::{SURFACE_PROBE_BALL_DIAMETER_MM, SURFACE_PROBE_BALL_LENGTH_MM};
pub use emission::{
    ENTRY_RAMP_MIN_BITE_MM, PencilLinkReport, entry_bite_budget_mm, entry_ramp_window_mm,
    tip_contact_radius,
};
pub(crate) use emission::{emit_paths_with_entry_stock, plan_entry_ramp};

use tracing::info;

use crate::compute::config::TipFloatFinding;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;

use chain_paths::reach_gap_at_point;
use detectors::{curvature_arm, dihedral_arm, rest_depth_arm};
use emission::emit_paths_with_entry_stock_reported;

/// Which valley-detection front-end the pencil generator uses.
///
/// The project-file and MCP token is the snake-case variant name:
/// `dihedral`, `curvature`, `rest_depth`. An unknown token refuses the load
/// and names the key, as every other typed operation dial does (FIN-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PencilDetector {
    /// Mesh-dihedral crease detection (Ohtake-Belyaev-Seidel-style discrete
    /// curvature creases). Robust on clean CAD-style meshes with sharp internal
    /// corners; on dense noisy relief it fires on every triangulation crease and
    /// fragments. The historical default.
    #[default]
    Dihedral,
    /// Curvature crest-line extraction (Ohtake-Belyaev-Seidel 2004 / Yoshizawa
    /// 2005, curvature via Rusinkiewicz 2004): per-vertex principal curvatures →
    /// minimal-curvature extremality → zero-crossing valley lines, filtered by a
    /// single `valley_saliency` (|κ₂|) dial. Traces every concave seam on dense
    /// organic relief and dials cleanly from "all seams" to "deep sharp valleys
    /// only". See [`crate::finish::crest_lines`]. The right choice for noisy meshes.
    Curvature,
    /// Rest-depth-field detection (the tool-offset-space one; see
    /// [`crate::surface::rest_field`]). Computes `rest = drop_z(reference) − drop_z(pencil)`
    /// on an XY grid — the dual-tool comparison every commercial CAM uses — and
    /// traces the skeleton of each rest region. Unlike the other three detectors
    /// it is NOT tool-radius-blind: the field is zero wherever the reference tool
    /// already reached, and the `reference_tool_diameter` dial visibly moves the
    /// detection. Routes narrow regions to pencil centrelines and wide regions to
    /// clearing. The aligned detector — recommended on relief.
    RestDepth,
}

/// Parameters for pencil finishing.
pub struct PencilParams {
    /// Dihedral angle threshold in degrees. Edges with concave angles below this
    /// are considered creases. Default: 160° (nearly flat edges ignored).
    pub bitangency_angle: f64,
    /// Minimum chain length to keep (mm). Chains shorter than this are discarded.
    /// Default: tool diameter.
    pub min_cut_length: f64,
    /// Maximum gap between chain endpoints for linking (mm).
    /// Nearby chains are connected with rapid moves. Default: tool_diameter * 3.
    pub hookup_distance: f64,
    /// Number of offset passes on each side of the centerline. 0 = centerline
    /// only. For `Dihedral`/`Curvature` this is the exact count used
    /// everywhere; for `RestDepth` it is instead a CAP — each chain narrows
    /// it to however many stepovers actually fit the local valley half-width
    /// (see [`rest_depth_arm`]), so a narrow crease gets fewer (or zero)
    /// offset passes even when this dial is set higher.
    pub num_offset_passes: usize,
    /// Offset stepover between parallel passes (mm). Default: tool_radius * 0.5.
    pub offset_stepover: f64,
    /// Point spacing along paths (mm). Default: 0.5mm.
    pub sampling: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves.
    pub safe_z: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
    /// Reach-gap tolerance (mm): minimum uncut depth at a concave seam for it to
    /// count as a genuine valley worth cutting. A candidate edge is kept only
    /// where the tool's resting reference floats more than this above the true
    /// surface (i.e. the tool bridges the valley). Raise it to ignore shallow
    /// surface texture and keep only deeper channels; lower it to catch fine
    /// detail. Default 0.05mm (see [`reach_gap_threshold`]).
    pub min_valley_depth: f64,
    /// Bisector positioning strength (0 = off, 1 = geometrically correct). In an
    /// asymmetric internal corner (steep wall + flat floor, e.g. a lake edge) the
    /// seam line is NOT where a vertical ball nestles — it rides up the steep wall
    /// and leaves the fillet uncut. This shifts the trace X,Y out along the
    /// bisector of the two walls (zero for symmetric valleys) so the drop rests
    /// tangent to both. 1.0 = the exact `r·(n1+n2)/(1+n1·n2)` offset; lower to
    /// under-shoot, raise to over-shoot when dialing. Default 1.0
    /// (see [`bisector_strength_default`]).
    pub bisector_strength: f64,
    /// Diameter (mm) of the bigger *reference* tool the pencil pass cleans up after
    /// (typically the finishing tool). The gate keeps a seam by how much DEEPER the
    /// pencil tool reaches than this reference could —
    /// `rest_depth = reference_gap − pencil_gap` — so it traces the valleys the big
    /// tool missed but the pencil tool can enter, and skips both big-tool-reachable
    /// walls and sub-pencil-scale texture (where both tools float). When `<=` the
    /// pencil tool's own diameter, falls back to the self-referenced gap. Default 6.0
    /// (see [`reference_tool_diameter_default`]).
    pub reference_tool_diameter: f64,
    /// Which valley-detection algorithm to use (see [`PencilDetector`]). Default
    /// `Dihedral` (crease detection); `Curvature` extracts curvature crest lines
    /// and is the right choice for noisy organic relief.
    pub detector: PencilDetector,
    /// Minimum concave curvature |κ₂| (1/mm) a valley must reach to be traced by
    /// the `Curvature` detector — THE significance dial. Low → every concave
    /// seam; high → only deep sharp valleys (a flat basin has κ₂ ≈ 0 and drops
    /// out at any positive value). Default 0.05 (see [`valley_saliency_default`]).
    pub valley_saliency: f64,
    /// Curvature-tensor smoothing iterations for the `Curvature` detector — the
    /// literature denoise (smooths the curvature field, not the geometry). More
    /// suppresses triangulation noise at the cost of blurring nearby valleys.
    /// Default 3 (see [`curvature_smoothing_default`]).
    pub curvature_smoothing: usize,
    /// XY grid cell size (mm) for the `RestDepth` detector's rest field. Smaller
    /// = finer regions and more drops. Default 0.5 (see [`rest_cell_default`]).
    pub rest_cell_mm: f64,
    /// R1: a real reference tool (from the library) whose *true* cutter geometry
    /// defines the rest reference, shared by all three detectors. `Some`
    /// overrides the nominal `reference_tool_diameter` ball — a flat end mill,
    /// vbit, or tapered ball leaves a completely different rest shape than a ball
    /// of the same diameter. `None` = legacy nominal-diameter behaviour.
    /// `ToolDefinition` implements `MillingCutter`, so it drops directly.
    pub reference_cutter: Option<crate::tool::ToolDefinition>,
    /// P1 quantitative linker (unified-finishing-pass W4a): the machine
    /// envelope [`emit_paths`] costs a surface-link candidate against a
    /// retract-link candidate with, using the F-034 cycle-time
    /// integrator ([`crate::machine::kinematics::surface_link_time`] /
    /// [`crate::machine::kinematics::retract_link_time`]). `hookup_distance`
    /// remains the candidate CAP — a gap must still be within it to be
    /// considered for a surface link at all — but which link actually
    /// gets emitted is decided by integrated time, not distance, once
    /// this is `Some`. `None` keeps the legacy behaviour: emit a surface
    /// link whenever `build_surface_link` succeeds within
    /// `hookup_distance`.
    pub link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    /// G-LINKSTAGE: a SEPARATE, usually shorter, cap on the gap a CLEARANCE
    /// HOP may span — the link that lifts clear of standing material and
    /// descends again ([`emission::plan_link_lift`]). `None` (the shipped value) means
    /// "the same cap as [`Self::hookup_distance`]", which is byte-identical
    /// to the pre-stage emitter.
    ///
    /// # Why the two caps must differ
    ///
    /// The two link tiers do NOT buy the same thing, and the measurement
    /// says so (`planning/pencil_linking_2026-09-04.md`, "REACH LEVER
    /// FALSIFIED"). A link that arrives AT CUTTING DEPTH removes the next
    /// fragment's entry outright — and on this pass an entry is 7.2 s
    /// against 0.16 s of cutting, so that is the whole prize. A link that
    /// LIFTS still lands from above, so the fragment still pays
    /// [`emission::emit_entry_descent`], and the lift itself has to be travelled.
    ///
    /// With one cap governing both, widening it to reach more at-depth
    /// candidates drags in long candidates that cross finished terrain,
    /// which G-LINKLOAD then lifts: on realistic after-scallop stock, 5 mm →
    /// 30 mm made the pass **4.2× worse** (4 985 s → 20 997 s), with
    /// `tip_float` 528 → 4 917. Splitting the caps is what lets the at-depth
    /// reach grow without buying that.
    ///
    /// The operator dial is
    /// [`crate::compute::operation_configs::PencilConfig::link_hop_distance_mm`],
    /// which `execute.rs` copies here. `Some(0.0)` refuses every hop and
    /// keeps the at-depth tier, which is the control arm that measures what
    /// each tier is worth on its own.
    pub link_hop_distance_mm: Option<f64>,
}

/// Sensible test/prototyping defaults, sourced from the field-level
/// `*_default()` fns documented above where one exists (`min_valley_depth`,
/// `bisector_strength`, `reference_tool_diameter`, `detector`,
/// `valley_saliency`, `curvature_smoothing`, `rest_cell_mm`) and from the
/// doc comments' stated defaults or the
/// most common test literal otherwise. Production callers (`execute.rs`)
/// build every field explicitly from `PencilConfig`, so this exists purely
/// to collapse test literal blocks via `..Default::default()` — it's never
/// on the production path.
impl Default for PencilParams {
    fn default() -> Self {
        Self {
            bitangency_angle: 160.0,
            min_cut_length: 2.0,
            hookup_distance: 5.0,
            num_offset_passes: 0,
            offset_stepover: 0.5,
            sampling: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
            min_valley_depth: reach_gap_threshold(),
            bisector_strength: bisector_strength_default(),
            reference_tool_diameter: reference_tool_diameter_default(),
            detector: PencilDetector::default(),
            valley_saliency: valley_saliency_default(),
            curvature_smoothing: curvature_smoothing_default(),
            rest_cell_mm: rest_cell_default(),
            reference_cutter: None,
            link_kinematics: None,
            // "Same cap as `hookup_distance`" — byte-identical.
            link_hop_distance_mm: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PencilRuntimeEvent {
    OffsetPass {
        chain_index: usize,
        chain_total: usize,
        offset_index: usize,
        offset_total: usize,
        offset_mm: f64,
        is_centerline: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PencilRuntimeAnnotation {
    pub move_index: usize,
    pub event: PencilRuntimeEvent,
}

impl PencilRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::OffsetPass {
                chain_index,
                offset_index,
                is_centerline,
                ..
            } => {
                if *is_centerline {
                    format!("Chain {chain_index} centerline")
                } else {
                    format!("Chain {chain_index} offset pass {offset_index}")
                }
            }
        }
    }
}

/// `pub` (not `pub(crate)`) so [`crate::finish::crease_paths::centerline_cut_paths`]
/// can be `pub` too and name `Vec<PencilPath>` as its return type — C9
/// (`tests/per_point_claims_fan_c9.rs`) needs a caller OUTSIDE the crate
/// able to hand `centerline_cut_paths` a hand-built `RestCenterline` (exact
/// per-point depth, not detector output) and inspect exactly what came out.
/// Fields stay private; read them through the accessors below. Every
/// in-crate caller of `centerline_cut_paths` (`rest_depth_arm`,
/// `unified_finish.rs`'s claims pass) only forwards the `Vec`
/// [`paths_from_sampled`] fills in — it never constructs or reads a
/// `PencilPath` itself, so this widening does not change how they behave.
#[derive(Clone)]
pub struct PencilPath {
    points: Vec<P3>,
    chain_index: usize,
    chain_total: usize,
    offset_index: usize,
    offset_total: usize,
    offset_mm: f64,
    is_centerline: bool,
}

impl PencilPath {
    /// The lifted, ordered points of this pass. `Z == f64::NAN` marks a
    /// point the tool cannot contact here — off the mesh
    /// ([`chain_paths::lift_to_surface`]) or, for an offset pass, truncated because the
    /// local reach does not support it at this offset (see
    /// [`paths_from_sampled`]'s `OffsetFan::reach` doc). Non-`NaN` points
    /// are real cutting contact.
    #[must_use]
    pub fn points(&self) -> &[P3] {
        &self.points
    }

    /// This pass's lateral offset from the centreline (mm), signed:
    /// positive is the `fan.left` side, negative `fan.right`. For a
    /// per-point fan (C9 — [`OffsetFan::stepover`] non-empty) the true
    /// offset varies point to point, so this is the MEAN across the pass's
    /// points, a summary for labelling/diagnostics, not the exact position
    /// of any one point — read [`Self::points`] for that. `0.0` for the
    /// centreline pass.
    #[must_use]
    pub fn offset_mm(&self) -> f64 {
        self.offset_mm
    }

    /// `true` for the centreline pass, `false` for every offset pass.
    #[must_use]
    pub fn is_centerline(&self) -> bool {
        self.is_centerline
    }
}

/// Default bisector positioning strength (1.0 = the geometrically-correct offset).
pub(crate) fn bisector_strength_default() -> f64 {
    1.0
}

/// Default reference-tool diameter (mm): the bigger finishing tool the pencil pass
/// cleans up after. A typical 1/4" finish ball.
pub(crate) fn reference_tool_diameter_default() -> f64 {
    6.0
}

/// Default valley saliency (min |κ₂| in 1/mm) for the curvature detector. 0.15
/// traces the full dendritic valley network on dense organic relief without
/// carpeting the inter-valley wave/triangulation texture (validated on the
/// wanaka rivermap mesh: ~0.05 carpets, ~0.2 the coherent network, ~0.8 only the
/// deepest trunks). Raise to keep only deep sharp valleys, lower to trace finer
/// seams.
pub(crate) fn valley_saliency_default() -> f64 {
    0.15
}

/// Default curvature-tensor smoothing iterations for the curvature detector —
/// enough to suppress rivermap wave/triangulation texture while preserving
/// genuine valleys.
pub(crate) fn curvature_smoothing_default() -> usize {
    4
}

/// Default XY cell size (mm) for the rest-depth field. 0.5 mm gives ~160k drops
/// on the 200 mm wanaka mesh (release: ~2 s) with fine enough regions.
pub(crate) fn rest_cell_default() -> f64 {
    0.5
}

/// Keep only chains that sit in genuine rest material (see
/// [`polyline_passes_depth`] — the mesh-vertex chain is converted to its P3
/// vertex positions and gated with the same median-of-≤8-samples metric the
/// P3-native detectors use). The hot path on dense meshes is the
/// `drop_cutter` work, so gating whole chains (a handful of sample drops
/// each) instead of every candidate edge slashes the drop count.
/// Parallelised across cores when the `parallel` feature is on
/// (`MillingCutter: Send + Sync`).
fn gate_chains_by_depth(
    chains: Vec<Vec<u32>>,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    reference: Option<&dyn MillingCutter>,
    threshold: f64,
) -> Vec<Vec<u32>> {
    let keep = |c: &Vec<u32>| -> bool {
        // SAFETY: chain vertex indices come from `chain_concave_edges`, which
        // only ever walks vertex indices produced by this same mesh's edges.
        #[allow(clippy::indexing_slicing)]
        let pts: Vec<P3> = c.iter().map(|&i| mesh.vertices[i as usize]).collect();
        polyline_passes_depth(&pts, mesh, index, cutter, reference, threshold)
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        chains.into_par_iter().filter(keep).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        chains.into_iter().filter(keep).collect()
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Order chains by nearest-neighbor to minimize rapids.
fn order_paths_nearest(paths: &mut [PencilPath]) {
    if paths.len() <= 1 {
        return;
    }

    let mut ordered_indices = Vec::with_capacity(paths.len());

    // Was a Θ(chains²) scan over every unused chain (PERF_REVIEW G5). A chain
    // is measured by BOTH of its endpoints — the scan's
    // `d_start.min(d_end)` — so each chain registers two candidate points
    // under one owner id and `NearestPicker` takes the min for us (its
    // NaN-last ordering is `f64::min`'s). Chains with no points register
    // nothing, which is the scan's `continue`. `f64::MAX` — not
    // `f64::INFINITY` — is this site's acceptance sentinel, and index 0 its
    // fallback; both reproduced exactly, including the fallback's ability to
    // re-emit an already-used index when no chain clears the sentinel.
    let mut picker = crate::geometry::nn_order::NearestPicker::new(
        crate::geometry::nn_order::Metric::EuclidSq,
        paths.len(),
    );
    for (i, path) in paths.iter().enumerate() {
        let (Some(first), Some(last)) = (path.points.first(), path.points.last()) else {
            continue;
        };
        picker.push(i, first.x, first.y);
        picker.push(i, last.x, last.y);
    }
    picker.build();

    // Start with first chain
    ordered_indices.push(0);
    picker.remove(0);

    for _ in 1..paths.len() {
        let last_path = &paths[ordered_indices[ordered_indices.len() - 1]];
        let last_pt = if let Some(p) = last_path.points.last() {
            *p
        } else {
            continue;
        };

        let best_idx = match picker.nearest(last_pt.x, last_pt.y) {
            Some((i, d)) if d < f64::MAX => i,
            _ => 0,
        };

        // If end is closer than start, reverse the chain
        if !paths[best_idx].points.is_empty() {
            let start_pt = paths[best_idx].points[0];
            let end_pt = paths[best_idx].points[paths[best_idx].points.len() - 1];
            let d_start = (start_pt.x - last_pt.x).powi(2) + (start_pt.y - last_pt.y).powi(2);
            let d_end = (end_pt.x - last_pt.x).powi(2) + (end_pt.y - last_pt.y).powi(2);
            if d_end < d_start {
                paths[best_idx].points.reverse();
            }
        }

        // The reversal above rewrote this chain's endpoints, but it leaves the
        // picker at the same moment, so the registered points can never go
        // stale.
        picker.remove(best_idx);
        ordered_indices.push(best_idx);
    }

    // Reorder chains in-place using the ordering
    let mut temp: Vec<PencilPath> = ordered_indices
        .into_iter()
        .map(|i| {
            std::mem::replace(
                &mut paths[i],
                PencilPath {
                    points: Vec::new(),
                    chain_index: 0,
                    chain_total: 0,
                    offset_index: 0,
                    offset_total: 0,
                    offset_mm: 0.0,
                    is_centerline: false,
                },
            )
        })
        .collect();
    for (i, path) in temp.drain(..).enumerate() {
        paths[i] = path;
    }
}

/// Generate pencil finishing toolpath.
///
/// Detects concave mesh edges (creases) and generates toolpaths that
/// follow them, cleaning material that standard finishing passes miss.
pub fn pencil_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
) -> Toolpath {
    let (tp, _, _) = pencil_toolpath_structured_annotated(mesh, index, cutter, params, None, None);
    tp
}

impl crate::compute::spans::RuntimeLabel for PencilRuntimeAnnotation {
    fn move_index(&self) -> usize {
        self.move_index
    }

    fn label(&self) -> String {
        self.event.label()
    }
}

/// The shared rest-depth gate (P1.5): does a P3 polyline sit in genuine rest
/// material the pencil tool can clean? Samples up to 8 points and keeps the
/// line if the MEDIAN *rest depth* (`reference_gap − pencil_gap`, i.e. how
/// much deeper the pencil tool reaches than the bigger `reference` tool
/// could) exceeds `threshold`. The surface height cancels in reference mode,
/// so it tolerates the smoothed DEM Z curvature/rest-depth lines carry. Used
/// directly by the `Curvature`/`RestDepth` detectors, and by
/// [`gate_chains_by_depth`] for the `Dihedral` detector's mesh-vertex chains
/// (converted to points first — same metric, same threshold, one
/// implementation instead of two near-identical copies).
fn polyline_passes_depth(
    pts: &[P3],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    reference: Option<&dyn MillingCutter>,
    threshold: f64,
) -> bool {
    let n = pts.len();
    if n == 0 {
        return false;
    }
    let step = (n / 8).max(1);
    let mut depths: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < n {
        #[allow(clippy::indexing_slicing)] // i < n by loop guard
        let v = pts[i];
        if let Some(pencil_gap) = reach_gap_at_point(v.x, v.y, v.z, mesh, index, cutter) {
            let rest = match reference {
                Some(rc) => match reach_gap_at_point(v.x, v.y, v.z, mesh, index, rc) {
                    Some(ref_gap) => (ref_gap - pencil_gap).max(0.0),
                    None => pencil_gap,
                },
                None => pencil_gap,
            };
            depths.push(rest);
        }
        i += step;
    }
    if depths.is_empty() {
        return false;
    }
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    #[allow(clippy::indexing_slicing)] // depths non-empty → index < len
    let median = depths[depths.len() / 2];
    median > threshold
}

/// What one pencil pass MEASURED on its way to the toolpath (FIN-12).
///
/// Four `&mut Option<..>` out-parameters carried these values before, so a
/// caller had to declare four bindings, remember their order and read the
/// doc to learn which ones a given detector sets.
/// [`crate::finish::unified_finish::UnifiedFinishReport`] already solved the
/// same problem by returning a report, and this is that shape.
///
/// Every field keeps the `None` reading it had as an out-parameter:
/// `None` is **not measured**, never a measured zero.
#[derive(Debug, Clone, Default)]
pub struct PencilReport {
    /// The `RestDepth` detector's rest-field grid, for the GUI heatmap
    /// overlay. `None` for every other detector — no rest field was built.
    pub rest_grid: Option<crate::surface::rest_field::RestGrid>,
    /// The `RestDepth` detector's derived machining-region polygons (P2.2
    /// selective-finishing boundary source). `None` for every other
    /// detector.
    pub rest_regions: Option<Vec<Polygon2>>,
    /// Wave D1: the centreline TIP-FLOAT tally. Always `Some` from the
    /// cancellable entry point — a detector that emitted no centreline
    /// still measured zero points, which is a different statement from
    /// "not measured".
    pub tip_float: Option<TipFloatFinding>,
    /// G-LINKVISIBLE: what the link stage did. `Some` ONLY where the
    /// emitter produced it, so a detector that found no centreline leaves
    /// it `None` — the emitter, and with it every junction decision, never
    /// ran.
    pub link: Option<PencilLinkReport>,
}

#[allow(clippy::expect_used)]
pub fn pencil_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    // R2: the actual machined stock left by prior toolpaths, when this pencil op
    // cuts `FromRemainingStock` and a prior simulation exists. The RestDepth
    // detector prefers it as the rest reference (it captures the real prior
    // toolpath pattern). `None` ⇒ fall back to the tool/nominal reference (R1).
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>, PencilReport) {
    crate::interrupt::run_uncancellable(|cancel| {
        pencil_toolpath_structured_annotated_with_cancel(
            mesh,
            index,
            cutter,
            params,
            initial_stock,
            debug,
            cancel,
        )
    })
}

/// Cancellable variant of [`pencil_toolpath_structured_annotated`]. Polls
/// `cancel` between the major phases (detector dispatch → path ordering →
/// emission) and once per line/chain inside each detector arm's own sampling
/// loop (see [`dihedral_arm`], [`curvature_arm`], [`rest_depth_arm`]). Does
/// NOT poll inside the detector internals themselves
/// (`crest_lines::detect_valley_lines`, `rest_field::detect_rest_valleys`,
/// `chain_concave_edges`/`gate_chains_by_depth`) — those stay as a follow-up
/// if they ever show up as the actual long pole.
pub fn pencil_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    debug: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<PencilRuntimeAnnotation>, PencilReport), Cancelled> {
    check_cancel(cancel)?;
    let tp = Toolpath::new();
    let annotations = Vec::new();
    let mut float = TipFloatFinding::default();
    let mut report = PencilReport::default();

    let mut all_paths: Vec<PencilPath> = match params.detector {
        PencilDetector::Curvature => {
            curvature_arm(mesh, index, cutter, params, &mut float, cancel)?
        }
        PencilDetector::RestDepth => rest_depth_arm(
            mesh,
            index,
            cutter,
            params,
            initial_stock,
            debug,
            &mut report,
            &mut float,
            cancel,
        )?,
        PencilDetector::Dihedral => dihedral_arm(mesh, index, cutter, params, &mut float, cancel)?,
    };
    // Wave D1: the detector always measures float, so the tally is `Some`
    // even on a pass that emitted no centreline.
    report.tip_float = Some(float);

    if all_paths.is_empty() {
        return Ok((tp, annotations, report));
    }
    check_cancel(cancel)?;

    // Step 6: Order paths by nearest-neighbor
    order_paths_nearest(&mut all_paths);
    check_cancel(cancel)?;

    // Step 7: Emit toolpath. The input stock goes in so entries can ramp
    // along the crease instead of carving down into it (G-ENTRYLOAD).
    let (tp, annotations, link_report) = emit_paths_with_entry_stock_reported(
        &all_paths,
        mesh,
        index,
        cutter,
        params,
        initial_stock,
    );
    // G-LINKVISIBLE: recorded even when every counter is zero — the emitter
    // ran, so the measurement exists.
    report.link = Some(link_report);

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    info!(
        moves = tp.moves.len(),
        paths = all_paths.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        "Pencil toolpath complete"
    );

    Ok((tp, annotations, report))
}

#[cfg(test)]
mod tests;
