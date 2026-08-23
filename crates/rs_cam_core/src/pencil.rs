//! Pencil finishing — traces concave edges (creases) on mesh surfaces.
//!
//! Owns orchestration (reference-tool resolution, detector dispatch) and the
//! shared pipeline every detector arm feeds into: fair → lift-to-surface →
//! rest-depth gate → offset passes → nearest-neighbor order → emit. The
//! three valley-detection front-ends themselves ([`PencilDetector`]) each
//! live in their own module, mirrored on the [`crate::crest_lines`] /
//! [`crate::rest_field`] pattern:
//! - **Dihedral** (the historical default) — [`crate::pencil_dihedral`]:
//!   mesh-crease detection via per-edge dihedral angle + graph chaining.
//! - **Curvature** — [`crate::crest_lines`]: curvature crest-line extraction,
//!   the right choice for dense noisy organic relief.
//! - **RestDepth** — [`crate::rest_field`]: the tool-radius-aware dual-tool
//!   rest field.
//!
//! Each arm ([`dihedral_arm`], [`curvature_arm`], [`rest_depth_arm`]) turns
//! its detector's raw output into `PencilPath`s via the shared
//! [`paths_from_sampled`]; [`pencil_toolpath_structured_annotated_with_cancel`]
//! dispatches to the right arm, then orders and emits.

use std::collections::HashMap;

use tracing::{info, warn};

use crate::compute::config::TipFloatFinding;
use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P3, V3, polyline_length};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::pencil_dihedral::{
    EdgeKey, SharedEdge, build_edge_adjacency, chain_concave_edges, compute_shared_edges,
    sample_chain_bisected,
};
use crate::polygon::Polygon2;
use crate::surface_link::build_surface_link;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Which valley-detection front-end the pencil generator uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    /// only". See [`crate::crest_lines`]. The right choice for noisy meshes.
    Curvature,
    /// Rest-depth-field detection (the tool-offset-space one; see
    /// [`crate::rest_field`]). Computes `rest = drop_z(reference) − drop_z(pencil)`
    /// on an XY grid — the dual-tool comparison every commercial CAM uses — and
    /// traces the skeleton of each rest region. Unlike the other three detectors
    /// it is NOT tool-radius-blind: the field is zero wherever the reference tool
    /// already reached, and the `reference_tool_diameter` dial visibly moves the
    /// detection. Routes narrow regions to pencil centrelines and wide regions to
    /// clearing. The aligned detector — recommended on relief.
    RestDepth,
}

impl PencilDetector {
    /// Parse from a lowercase config string; unknown values fall back to the
    /// default (Dihedral) so older project files keep loading.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "curvature" | "crest" | "ridgevalley" | "ridge_valley" => PencilDetector::Curvature,
            "rest_depth" | "restdepth" | "rest" => PencilDetector::RestDepth,
            _ => PencilDetector::Dihedral,
        }
    }

    /// Canonical lowercase token for serialisation.
    pub fn as_str(&self) -> &'static str {
        match self {
            PencilDetector::Dihedral => "dihedral",
            PencilDetector::Curvature => "curvature",
            PencilDetector::RestDepth => "rest_depth",
        }
    }
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
    /// **RETIRED (PR-5, H2.2) — carried, reported, and NOT READ.**
    ///
    /// The `RestDepth` pencil/clearing decision is now the coverage criterion
    /// in [`crate::reach`]. See
    /// [`crate::compute::operation_configs::PencilConfig::route_width_factor`]
    /// for why the field still exists and how a non-default value is
    /// surfaced. Default 2.0 (see [`route_width_factor_default`]).
    pub route_width_factor: f64,
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
    /// integrator ([`crate::machine_kinematics::surface_link_time`] /
    /// [`crate::machine_kinematics::retract_link_time`]). `hookup_distance`
    /// remains the candidate CAP — a gap must still be within it to be
    /// considered for a surface link at all — but which link actually
    /// gets emitted is decided by integrated time, not distance, once
    /// this is `Some`. `None` keeps the legacy behaviour: emit a surface
    /// link whenever `build_surface_link` succeeds within
    /// `hookup_distance`.
    pub link_kinematics: Option<crate::machine_kinematics::LinkKinematics>,
}

/// Sensible test/prototyping defaults, sourced from the field-level
/// `*_default()` fns documented above where one exists (`min_valley_depth`,
/// `bisector_strength`, `reference_tool_diameter`, `detector`,
/// `valley_saliency`, `curvature_smoothing`, `rest_cell_mm`,
/// `route_width_factor`) and from the doc comments' stated defaults or the
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
            route_width_factor: route_width_factor_default(),
            reference_cutter: None,
            link_kinematics: None,
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

/// `pub` (not `pub(crate)`) so [`crate::crease_paths::centerline_cut_paths`]
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
    /// ([`lift_to_surface`]) or, for an offset pass, truncated because the
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

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate an offset polyline by shifting each point perpendicular to the
/// path direction in XY, by a PER-POINT offset distance (`offsets[i]` for
/// `points[i]`; a short `offsets` reads the missing tail as `0.0`).
///
/// # Why variable, not just scalar (C9)
///
/// A pencil fan's stepover used to be one constant applied to every point on
/// a pass ([`offset_polyline`], now a thin `offset == offsets[i]` caller of
/// this). That is still correct for the Dihedral/Curvature arms and for a
/// centreline the reach policy never measured ([`OffsetFan::stepover`]
/// empty), but a MEASURED centreline's own working width is depth-dependent
/// ([`crate::reach::suggested_offset_stepover_mm`]), so a branch that runs
/// shallow at one end and deep at the other has no single honest stepover —
/// see [`paths_from_sampled`]'s doc. One geometry implementation serves both
/// cases; only the offset each point reads differs.
fn offset_polyline_variable(points: &[P3], offsets: &[f64]) -> Vec<P3> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len());

    for i in 0..points.len() {
        // Compute tangent direction at this point
        let tangent = if i == 0 {
            let d = points[1] - points[0];
            nalgebra::Vector2::new(d.x, d.y)
        } else if i == points.len() - 1 {
            let d = points[i] - points[i - 1];
            nalgebra::Vector2::new(d.x, d.y)
        } else {
            let d = points[i + 1] - points[i - 1];
            nalgebra::Vector2::new(d.x, d.y)
        };

        let len = tangent.norm();
        if len < 1e-10 {
            result.push(points[i]);
            continue;
        }

        // Perpendicular direction in XY (rotate tangent 90° CCW)
        let normal = nalgebra::Vector2::new(-tangent.y, tangent.x) / len;
        let offset = offsets.get(i).copied().unwrap_or(0.0);

        result.push(P3::new(
            points[i].x + normal.x * offset,
            points[i].y + normal.y * offset,
            points[i].z,
        ));
    }

    result
}

/// Generate an offset polyline by shifting each point perpendicular to the
/// path direction in XY by the SAME offset distance. Thin caller of
/// [`offset_polyline_variable`] — see its doc for why the variable form
/// exists.
fn offset_polyline(points: &[P3], offset: f64) -> Vec<P3> {
    offset_polyline_variable(points, &vec![offset; points.len()])
}

/// Default fairing strength: how far each interior point moves toward the
/// midpoint of its neighbours per pass (0 = none, 1 = full Laplacian step).
pub(crate) const FAIRING_STRENGTH: f64 = 0.5;
/// Default number of fairing passes applied to each sampled chain.
pub(crate) const FAIRING_PASSES: usize = 2;

/// Lightly fair a sampled polyline in XY to remove facet-scale jaggedness. The
/// raw chain hops between 0.5mm mesh-edge vertices, so the centerline zig-zags
/// at triangulation scale; a couple of Laplacian passes straighten it without
/// moving it materially off the valley floor. Z is left untouched — the
/// subsequent `lift_to_surface` drop re-solves it gouge-safely from the faired
/// X,Y — and endpoints are pinned so chains don't shrink at their tips.
#[allow(clippy::indexing_slicing)] // i bounded to 1..len-1; neighbours i±1 valid
fn fair_polyline_xy(points: &[P3], passes: usize, strength: f64) -> Vec<P3> {
    if points.len() < 3 || passes == 0 || strength <= 0.0 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    for _ in 0..passes {
        let prev = pts.clone();
        for i in 1..prev.len() - 1 {
            let a = prev[i - 1];
            let b = prev[i];
            let c = prev[i + 1];
            // Move toward the midpoint of the two neighbours (Laplacian).
            pts[i].x = b.x + strength * ((a.x + c.x) * 0.5 - b.x);
            pts[i].y = b.y + strength * ((a.y + c.y) * 0.5 - b.y);
        }
    }
    pts
}

/// Default bisector positioning strength (1.0 = the geometrically-correct offset).
pub(crate) fn bisector_strength_default() -> f64 {
    1.0
}

/// Lift 2D polyline points to the mesh surface using drop-cutter. The per-point
/// drop is the hot work, so it runs in parallel when the `parallel` feature is on.
///
/// Points where the cutter makes no contact (off the mesh — this happens when
/// bisector-shifted offset points get pushed past the mesh edge) come back
/// with `z = f64::NAN` instead of the original, un-lifted Z. This makes
/// non-contact explicit so the emit loop can split the polyline at the gap
/// instead of stitching a cutting move across missing material.
fn lift_to_surface(
    points: &[P3],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
) -> Vec<P3> {
    let lift = |p: &P3| {
        let cl = point_drop_cutter(p.x, p.y, mesh, index, cutter);
        if cl.contacted {
            P3::new(p.x, p.y, cl.z + stock_to_leave)
        } else {
            // Outside mesh — mark non-contact explicitly so the emit loop
            // splits the pass here instead of bridging the gap.
            P3::new(p.x, p.y, f64::NAN)
        }
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        points.par_iter().map(lift).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        points.iter().map(lift).collect()
    }
}

/// Tool-radius-aware reach gap at a concave edge's midpoint: how far the tool's
/// resting reference floats above the true surface there.
///
/// The edge midpoint lies on the mesh surface, so its Z *is* the true surface
/// height at that (x,y) — no ray needed. Dropping the actual cutter at the same
/// (x,y) gives the footprint-aware rest height (`cl.z`): on a flat or gentle
/// slope, or a triangulation-scale crease the tool simply rides over, the tool
/// reaches the surface and `gap ≈ 0`; in a concavity tighter than the tool
/// radius the tool bridges the walls and its reference floats above the floor,
/// so `gap > 0` (verified: a 6mm ball over a 0.5-slope V-valley rests 0.354mm
/// above the seam). This makes detection scale-aware — reject mesh noise, keep
/// genuine valleys — instead of trusting raw per-edge dihedral. Returns `None`
/// if the cutter makes no contact at the midpoint.
fn reach_gap_at_point(
    x: f64,
    y: f64,
    surf_z: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) -> Option<f64> {
    let cl = point_drop_cutter(x, y, mesh, index, cutter);
    if !cl.contacted {
        return None;
    }
    Some(cl.z - surf_z)
}

/// Surface tolerance (mm): uncut depth at a concave seam above which we treat it
/// as genuine rest material worth a pencil pass. ABSOLUTE, not scaled to the tool
/// — a large tool that bridges a valley leaving e.g. 0.3mm should still be
/// cleaned, while triangulation-scale creases the tool rides over leave
/// sub-tolerance gaps and are rejected. Any noise that slips through is further
/// removed by `min_cut_length` chain filtering (isolated noise points don't form
/// long chains). Tunable; calibrate against the real mesh harness.
pub(crate) fn reach_gap_threshold() -> f64 {
    0.05
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

/// Default rest-region routing threshold: a region whose half-width exceeds
/// `2.0 × routing radius` (one pencil diameter) routes to clearing rather than a
/// single pencil centreline.
pub(crate) fn route_width_factor_default() -> f64 {
    2.0
}

/// Default detector token for project-file serde (the historical crease detector).
pub(crate) fn detector_string_default() -> String {
    PencilDetector::Dihedral.as_str().to_owned()
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
    let mut picker =
        crate::nn_order::NearestPicker::new(crate::nn_order::Metric::EuclidSq, paths.len());
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
    let (tp, _) = pencil_toolpath_structured_annotated(
        mesh, index, cutter, params, None, None, &mut None, &mut None,
    );
    tp
}

fn runtime_annotations_to_labels(annotations: &[PencilRuntimeAnnotation]) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

/// Resample a polyline to ~`spacing` mm between points (linear interpolation),
/// preserving the first and last vertices. Curvature crest lines arrive at
/// mesh-edge resolution; this decouples cut-point spacing from mesh density.
///
/// `pub(crate)`: also used by [`crate::crease_paths::centerline_cut_paths`].
#[allow(clippy::indexing_slicing)] // first/last + windows(2) indices are bounded
pub(crate) fn resample_polyline(points: &[P3], spacing: f64) -> Vec<P3> {
    if points.len() < 2 || spacing <= 1e-6 {
        return points.to_vec();
    }
    let mut out = vec![points[0]];
    // Distance already travelled past the last emitted point along the current
    // walk, so spacing is continuous across segment boundaries.
    let mut carry = 0.0;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        if seg < 1e-9 {
            continue;
        }
        let mut d = spacing - carry;
        while d < seg {
            let t = d / seg;
            out.push(P3::new(
                a.x + (b.x - a.x) * t,
                a.y + (b.y - a.y) * t,
                a.z + (b.z - a.z) * t,
            ));
            d += spacing;
        }
        carry = seg - (d - spacing);
    }
    let last = points[points.len() - 1];
    let need_last = out
        .last()
        .is_none_or(|p| (p.x - last.x).abs() > 1e-6 || (p.y - last.y).abs() > 1e-6);
    if need_last {
        out.push(last);
    }
    out
}

/// Build the centreline + offset `PencilPath`s for one already-sampled valley
/// polyline. Shared by all three detector arms: it fairs the XY line, lifts
/// it to the surface with the real cutter, and emits the centreline plus its
/// offset fan. `chain_index` is 1-based.
///
/// # The fan is ASYMMETRIC (PR-5, H2.2)
///
/// `fan` carries a count PER SIDE, not one count applied to both. The
/// Checkpoint A matrix measured left/right reach differing by up to 22×
/// (2.982 mm versus 0.136 mm on one asymmetric fixture, `§6`), and a single
/// scalar is forced to the narrow side, so the wide side was under-covered
/// BY CONSTRUCTION. The Dihedral/Curvature arms pass
/// `params.num_offset_passes` on both sides (unchanged behaviour); the
/// RestDepth arm (via [`crate::crease_paths::centerline_cut_paths`]) passes
/// what the reach policy resolved.
///
/// `fan.reach` additionally truncates each pass POINT BY POINT: a pass runs
/// only where the local reach supports its offset, and elsewhere its Z is
/// marked non-contact so [`contact_runs`] splits it into the runs that are
/// real. Truncating instead of dropping is what lets a branch that is
/// reachable at one end and pinched at the other keep the reachable part —
/// the per-sample depth statistic Checkpoint A ruled for (§8.5). An empty
/// `reach` means "not measured": every pass runs full length, exactly as
/// before.
///
/// # The fan is also PER-POINT WIDE (C9)
///
/// `fan.stepover`, when non-empty, replaces the single `offset_stepover`
/// argument with one value per point: pass `k`'s offset at point `i` is
/// `sign * k * fan.stepover[i]`, not `sign * k * offset_stepover`. Before
/// C9 a MEASURED centreline (one with genuine [`RestCenterline::samples`])
/// still spaced its whole fan at one constant — usually sized from the
/// branch's shallowest reported depth
/// (`unified_finish.rs`'s `claims_offset_stepover_mm`,
/// `rest_depth_arm`'s `params.offset_stepover`) — even though
/// [`crate::reach::suggested_offset_stepover_mm`] is monotone
/// non-decreasing in depth, so every deeper point on the same branch got a
/// stepover too small for its own working width: under a fixed pass-count
/// cap, passes packed closer together than necessary instead of reaching
/// out, leaving the outer part of that point's reach uncovered
/// (`tests/per_point_claims_fan_c9.rs` measures the shortfall on the
/// shipped taper). `fan.reach` still does the per-point TRUNCATION — now
/// compared against `pass_num * fan.stepover[i]` instead of
/// `pass_num * offset_stepover` — so the coverage criterion
/// `k · stepover_i ≤ reach_i` holds pointwise, not just at the reference
/// depth the scalar was sized from.
///
/// An empty `fan.stepover` means "not measured", exactly like an empty
/// `fan.reach`: every pass falls back to the flat `offset_stepover`
/// argument for every point, unchanged from before C9. This is what the
/// Dihedral/Curvature arms and an unmeasured `RestCenterline`
/// (`without_samples`) still get via [`OffsetFan::symmetric`].
///
/// [`PencilPath::offset_mm`] cannot describe a per-point-wide pass with one
/// number; it becomes the MEAN of that pass's per-point offsets when
/// `fan.stepover` drove the emission, and stays the exact scalar offset
/// otherwise. See its doc.
///
/// Takes `stock_to_leave`/`offset_stepover` as plain scalars (rather than a
/// `&PencilParams`) so non-pencil callers — currently
/// [`crate::crease_paths::centerline_cut_paths`] — don't need a full
/// `PencilParams` just to emit cut paths. `pub(crate)` for that same reason.
///
/// # Tip float (Wave D1)
///
/// `float` accumulates the per-point TIP-FLOAT tally for the centreline this
/// call emits — see [`TipFloatFinding`]. It is measured here, and only here,
/// because this is the one place that both solves the cutter's resting Z and
/// still has the valley in hand; every detector arm and the unified-finish
/// crease node funnel through it, so one measurement covers all of them.
///
/// The valley floor is NOT `sampled[i].z`. The rest-depth arm's centrelines
/// already carry "Z from the pencil drop" (`RestCenterline::points`), so
/// differencing against them would measure zero by construction. The floor
/// is re-solved with the Ø0.1 mm surface probe ball the rest-depth reference
/// chain already uses for exactly this question, at the same XY. That probe
/// has its own (tiny) float in a sharp V, so the reported residual is a
/// slight UNDER-estimate — never an over-claim.
#[allow(clippy::too_many_arguments)] // cohesive per-chain emit; splitting hurts clarity
pub(crate) fn paths_from_sampled(
    sampled: &[P3],
    chain_index: usize,
    chain_total: usize,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    offset_stepover: f64,
    fan: OffsetFan<'_>,
    all_paths: &mut Vec<PencilPath>,
    float: &mut TipFloatFinding,
) {
    if sampled.len() < 2 {
        return;
    }
    // De-jag the facet-scale zig-zag before lifting; offsets derive from the
    // faired centreline so they inherit it.
    let sampled = fair_polyline_xy(sampled, FAIRING_PASSES, FAIRING_STRENGTH);
    // Per-point reach only applies when it lines up with the points it is
    // describing; fairing preserves the count, but a caller mismatch must
    // degrade to "not measured" rather than mis-attribute one point's reach
    // to another.
    let reach: &[crate::reach::Reach] = if fan.reach.len() == sampled.len() {
        fan.reach
    } else {
        &[]
    };
    // Same length-match discipline as `reach`, and the same fallback: a
    // caller mismatch (or a genuinely unmeasured centreline, which passes
    // an empty slice on purpose) degrades to the flat scalar rather than
    // reading past the end or mis-attributing one point's stepover to
    // another.
    let stepover: &[f64] = if fan.stepover.len() == sampled.len() {
        fan.stepover
    } else {
        &[]
    };
    let offset_total = 1 + fan.left + fan.right;

    let centerline = lift_to_surface(&sampled, mesh, index, cutter, stock_to_leave);
    // Wave D1 instrument. Offset passes are deliberately excluded: they are
    // MEANT to ride up the walls, so "float" is not a defect there.
    let probe =
        crate::tool::BallEndmill::new(SURFACE_PROBE_BALL_DIAMETER_MM, SURFACE_PROBE_BALL_LENGTH_MM);
    let valley_floor = lift_to_surface(&sampled, mesh, index, &probe, 0.0);
    for (tool_pt, floor_pt) in centerline.iter().zip(valley_floor.iter()) {
        // `stock_to_leave` is a commanded offset, not float — back it out so
        // a finishing allowance never reads as unreachable material.
        float.record((tool_pt.z - stock_to_leave) - floor_pt.z);
    }
    all_paths.push(PencilPath {
        points: centerline,
        chain_index,
        chain_total,
        offset_index: 1,
        offset_total,
        offset_mm: 0.0,
        is_centerline: true,
    });

    // Left fan, then right fan. Indices run sequentially rather than
    // even/odd because the two sides no longer have equal counts.
    let mut offset_index = 1usize;
    for (sign, passes) in [(1.0_f64, fan.left), (-1.0_f64, fan.right)] {
        for pass_num in 1..=passes {
            // Per-point offsets when the fan carries per-point stepovers
            // (C9); otherwise every point reads the same flat
            // `offset_stepover`, exactly as before. One geometry call
            // either way — `offset_polyline_variable` — so there is only
            // one implementation of "shift this polyline sideways".
            let (pts, offset_mm) = if stepover.is_empty() {
                let offset = sign * pass_num as f64 * offset_stepover;
                (offset_polyline(&sampled, offset), offset)
            } else {
                let offsets: Vec<f64> = stepover
                    .iter()
                    .map(|&s| sign * pass_num as f64 * s)
                    .collect();
                // `offset_mm` on the emitted path is a SUMMARY (the mean)
                // when the true offset varies per point — see
                // `PencilPath::offset_mm`'s doc.
                let mean = offsets.iter().sum::<f64>() / offsets.len().max(1) as f64;
                (offset_polyline_variable(&sampled, &offsets), mean)
            };
            let mut lifted = lift_to_surface(&pts, mesh, index, cutter, stock_to_leave);
            if !reach.is_empty() {
                for (i, p) in lifted.iter_mut().enumerate() {
                    // `want` is the offset THIS point's pass sits at: the
                    // per-point stepover when measured, the flat scalar
                    // otherwise — the same value that placed `p` in `pts`
                    // above, so the truncation test and the placement agree
                    // pointwise.
                    let want =
                        pass_num as f64 * stepover.get(i).copied().unwrap_or(offset_stepover);
                    let supported = reach.get(i).is_some_and(|r| {
                        !r.refused && (if sign > 0.0 { r.left_mm } else { r.right_mm }) >= want
                    });
                    if !supported {
                        // Same marker `lift_to_surface` uses for "no contact
                        // here"; `contact_runs` splits the pass on it.
                        p.z = f64::NAN;
                    }
                }
            }
            offset_index += 1;
            all_paths.push(PencilPath {
                points: lifted,
                chain_index,
                chain_total,
                offset_index,
                offset_total,
                offset_mm,
                is_centerline: false,
            });
        }
    }
}

/// The offset fan one call to [`paths_from_sampled`] should emit.
///
/// Per-side counts plus the optional per-point reach that truncates them and
/// the optional per-point stepover that spaces them — see
/// [`paths_from_sampled`]'s doc for why all three exist.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OffsetFan<'a> {
    /// Passes to emit on the `+offset` side.
    pub left: usize,
    /// Passes to emit on the `-offset` side.
    pub right: usize,
    /// Per-point reach, aligned 1:1 with the polyline handed to
    /// [`paths_from_sampled`]. Empty = not measured; no truncation.
    pub reach: &'a [crate::reach::Reach],
    /// Per-point offset stepover (mm), aligned 1:1 with the polyline handed
    /// to [`paths_from_sampled`] (C9). Empty = not measured; every pass
    /// falls back to the flat `offset_stepover` argument for every point —
    /// see [`Self::symmetric`] and [`paths_from_sampled`]'s doc.
    pub stepover: &'a [f64],
}

impl OffsetFan<'_> {
    /// The legacy symmetric fan with no per-point truncation or spacing —
    /// what the Dihedral and Curvature detector arms emit, unchanged.
    pub(crate) fn symmetric(passes: usize) -> Self {
        Self {
            left: passes,
            right: passes,
            reach: &[],
            stepover: &[],
        }
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

/// Split a `lift_to_surface`-lifted polyline into contiguous on-surface runs.
/// A point with `z.is_nan()` marks a spot where the cutter made no contact
/// (off the mesh); it ends the current run and is dropped rather than kept —
/// a single isolated non-contact point must not stitch its two on-surface
/// neighbors together with a cutting move. Runs of length < 2 are still
/// returned; callers skip those (mirrors the pre-split `len() < 2` guard).
fn contact_runs(points: &[P3]) -> Vec<&[P3]> {
    crate::point_runs::split_run_ranges(points, |_, p: &P3| !p.z.is_nan(), 1)
        .into_iter()
        .filter_map(|(s, e)| points.get(s..=e))
        .collect()
}

// ── Entry ramp (G-ENTRYLOAD, 2026-08-23) ────────────────────────────────
//
// A pencil pass that cannot link to its predecessor used to enter every run
// the same way: rapid to the run's first point at `safe_z`, then ONE straight
// fed descent to that point's finished Z. On a `FromRemainingStock` pencil
// that descent is a vertical carve through whatever the upstream op left
// standing over the crease — on the wanaka200 measurement, up to 3.72 mm of
// white oak taken by an R0.5 tapered-ball TIP, at the 150 mm/min flute-tip
// plunge cap, and 2,412 s of the op's 3,332 s total.
//
// It was also invisible. A pure-vertical descent samples as
// `CutKinematics::Plunge`, whose removal the simulator writes to
// `plunge_descent_mm` and NOT to `axial_engagement_mm` — so the
// crosses-standing caution (which reads the axial axis) never saw it, and the
// chipload/deflection/power gates drop entry spans wholesale via
// `tool_load::locality::is_steady_state_for_gate`. Nothing graded it. The
// second half of that hole is closed by `sim_triage::entry_load_finding`.
//
// The fix here is geometric: descend ALONG the valley being entered instead
// of into it, in bite-budgeted zig-zag laps, so no single lap can remove more
// than the budget below.

/// Fraction of the cutter's TIP (cusp) radius one entry lap may remove.
///
/// The budget is tool-scaled rather than pass-scaled because the generator
/// cannot know the pass's median bite — that only exists after a simulation.
/// The two are tied at the other end: `sim_triage::ENTRY_LOAD_MEDIAN_MULTIPLE`
/// (k = 2x the pass's own median body bite) grades what the entry ACTUALLY
/// removed, so a budget that is too generous for a given pass is reported
/// rather than hidden. On the wanaka200 pencil (R0.5 tip, 0.20 mm median
/// bite) this yields 0.25 mm = 1.25x the median, inside k.
pub const ENTRY_RAMP_BITE_TIP_FRACTION: f64 = 0.5;
/// Floor under the derived per-lap budget — a hair-thin tip must not be able
/// to generate an unbounded lap count.
pub const ENTRY_RAMP_MIN_BITE_MM: f64 = 0.10;
/// Ceiling over the derived per-lap budget. A big ball entering a shallow
/// crease still ramps rather than punching a full-diameter hole.
pub const ENTRY_RAMP_MAX_BITE_MM: f64 = 0.50;
/// Maximum ramp angle from horizontal, degrees.
///
/// This is deliberately NOT the plunge-rate cap. `stale.tapered_ball_plunge`
/// exists because a tapered ball END-cutting at its tip has zero surface
/// speed on the axis; a bounded-angle ramp is a peripheral cut, and the
/// literature/CAM convention for ball-family ramp entry is an angle limit
/// (a plunge is the 90 degree case). Vertical moves in the entry — only the
/// air descent down to the stock ceiling survives as one — still use
/// `params.plunge_rate`.
pub const ENTRY_RAMP_MAX_ANGLE_DEG: f64 = 8.0;
/// Shortest window (mm of path) worth ramping over. Below this the run is
/// treated as too short to enter along and the legacy descent is kept.
pub const ENTRY_RAMP_MIN_WINDOW_MM: f64 = 0.5;
/// Hard cap on laps. When the stock standing over a crease is so deep that
/// the budget would need more laps than this, the emission stays bounded and
/// the per-lap step grows past the budget — which the post-simulation
/// `project.entry_load` finding then reports. A silently unbounded ramp and a
/// silent plunge are the same failure.
pub const ENTRY_RAMP_MAX_LAPS: usize = 64;

/// The tool radius that actually nestles into a crease: the corner radius for
/// flat/bullnose cutters, the tip sphere (`cusp_radius_mm`) otherwise. For a
/// tapered ball `radius()` is the SHANK, which is why this is not it.
fn tip_contact_radius(cutter: &dyn MillingCutter) -> f64 {
    let cr = cutter.corner_radius_mm();
    if cr > 1e-6 {
        cr
    } else {
        cutter.cusp_radius_mm()
    }
}

/// Per-lap depth budget (mm) for a stepped entry ramp on a cutter with this
/// tip radius. See [`ENTRY_RAMP_BITE_TIP_FRACTION`].
pub fn entry_bite_budget_mm(tip_radius_mm: f64) -> f64 {
    (tip_radius_mm * ENTRY_RAMP_BITE_TIP_FRACTION)
        .clamp(ENTRY_RAMP_MIN_BITE_MM, ENTRY_RAMP_MAX_BITE_MM)
}

/// A planned entry manoeuvre for one run: an air-only vertical descent to the
/// input stock's ceiling, then bite-budgeted zig-zag laps along the run's own
/// first few millimetres.
struct EntryRampPlan {
    /// Z the vertical fed descent stops at — the conservative stock ceiling
    /// over the ramp window, so everything below it is cut by the laps and
    /// everything above it is air.
    air_descent_z: f64,
    /// Lap points in emission order. Every one is clamped to its own point's
    /// finished Z, so no lap can ever cut below the surface.
    points: Vec<P3>,
    /// Index into the run of the window's LAST point. The body pass resumes
    /// at `window_end + 1`; the final lap leaves every window point cut at
    /// its finished Z, so coverage is unchanged.
    window_end: usize,
    /// The per-lap Z step actually used (>= the budget only in the
    /// [`ENTRY_RAMP_MAX_LAPS`] clamp case).
    step_mm: f64,
}

/// Plan a bite-budgeted entry ramp along the first few millimetres of `run`.
///
/// Returns `None` — keeping the legacy single descent — when there is no
/// input stock reading, the run is too short to ramp along, or the stock over
/// the window already sits within one bite budget of the finished surface (in
/// which case the descent arrives in near-zero engagement and a ramp would
/// only cost time).
///
/// # Shape
///
/// `laps` descending zig-zag laps, each a straight ramp from the previous
/// level to the next across the whole window, then ONE flat lap at the floor.
/// The flat lap is not optional: a zig-zag whose last lap ramps down leaves a
/// wedge (up to one step) over the start of that lap, and coverage is sacred
/// — the campaign that took pencil coverage from 0.137 to 0.80 is not being
/// paid back a wedge per entry. A second flat lap is appended when parity
/// needs it, so the manoeuvre always ends at the FAR end of the window and
/// the body pass carries on forward from there.
///
/// # Why the worst bite is `2 x step`, not `step`
///
/// Two consecutive laps run in opposite directions, so their vertical gap is
/// widest at the turn: `2 x step` at one end, zero at the other. The window
/// is therefore sized so that `window_len x tan(angle) <= budget / 2`.
fn plan_entry_ramp(
    run: &[P3],
    stock: &crate::dexel_stock::TriDexelStock,
    contact_radius: f64,
    params: &PencilParams,
) -> Option<EntryRampPlan> {
    if run.len() < 2 {
        return None;
    }
    let tip = contact_radius.max(1e-6);
    let budget = entry_bite_budget_mm(tip);
    let tan_ramp = ENTRY_RAMP_MAX_ANGLE_DEG.to_radians().tan();
    let window_target = (budget / (2.0 * tan_ramp)).max(ENTRY_RAMP_MIN_WINDOW_MM);

    // The window: the prefix of the run out to `window_target` of XY travel,
    // with its cumulative arclength (both truncated at the same point, so
    // `arc` and the window slice index alike).
    let mut arc: Vec<f64> = Vec::with_capacity(run.len());
    let mut travelled = 0.0_f64;
    let mut prev: Option<P3> = None;
    for p in run {
        if let Some(q) = prev {
            travelled += ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt();
        }
        arc.push(travelled);
        prev = Some(*p);
        if travelled >= window_target {
            break;
        }
    }
    let window_end = arc.len().checked_sub(1)?;
    if window_end < 1 {
        return None;
    }
    let window = run.get(..=window_end)?;
    let window_len = *arc.last()?;
    if window_len <= 1e-6 {
        return None;
    }

    // The ceiling: how high the INPUT stock can stand anywhere under the tip
    // over the window. `max_conservative_top_z_in_disc` may only ever err
    // high, so the vertical descent below it is air by construction. The disc
    // is the TIP's, not the envelope's: an envelope-radius disc on a tapered
    // ball would read the valley RIM several millimetres away and ramp
    // through air that is not in the tool's way.
    let mut ceiling = f64::NEG_INFINITY;
    for p in window {
        if let Some(top) = stock.max_conservative_top_z_in_disc(p.x, p.y, contact_radius) {
            ceiling = ceiling.max(top);
        }
    }
    if !ceiling.is_finite() {
        return None;
    }
    let ceiling = ceiling.min(params.safe_z);
    let floor = window.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    if !floor.is_finite() {
        return None;
    }
    let depth = ceiling - floor;
    if depth <= budget {
        // Already inside the budget: the plain descent arrives in near-zero
        // engagement and is the cheaper motion.
        return None;
    }

    let per_lap = (window_len * tan_ramp).min(budget * 0.5).max(1e-6);
    let laps_f = (depth / per_lap)
        .ceil()
        .clamp(1.0, ENTRY_RAMP_MAX_LAPS as f64);
    let laps = laps_f as usize;
    let step = depth / laps as f64;

    let mut levels: Vec<(f64, f64)> = (0..laps)
        .map(|i| (ceiling - step * i as f64, ceiling - step * (i + 1) as f64))
        .collect();
    levels.push((floor, floor));
    if levels.len().is_multiple_of(2) {
        levels.push((floor, floor));
    }

    let mut points: Vec<P3> = Vec::with_capacity(levels.len() * window.len());
    for (lap, &(from_z, to_z)) in levels.iter().enumerate() {
        let forward = lap % 2 == 0;
        for k in 1..window.len() {
            let idx = if forward { k } else { window.len() - 1 - k };
            let p = window.get(idx)?;
            let a = *arc.get(idx)?;
            let frac = if forward {
                a / window_len
            } else {
                1.0 - a / window_len
            };
            let z = from_z + (to_z - from_z) * frac;
            points.push(P3::new(p.x, p.y, z.max(p.z)));
        }
    }

    Some(EntryRampPlan {
        air_descent_z: ceiling,
        points,
        window_end,
        step_mm: step,
    })
}

/// Emit the ordered `PencilPath`s as toolpath moves. Consecutive passes whose
/// endpoints are within `hookup_distance` are joined by a gouge-safe
/// surface-following feed instead of a retract-rapid-replunge — on dense
/// organic relief the chains fragment heavily, so per-fragment retracts
/// dominated the rapid distance. The retract is deferred: it fires only when
/// the next pass (or run — see [`contact_runs`]) is too far, or its link loses
/// surface contact, and once at the very end.
///
/// P1 quantitative linker (unified-finishing-pass W4a): `hookup_distance` is
/// now only the CANDIDATE cap — a gap has to be within it (and gouge-safe via
/// [`build_surface_link`]) to be considered at all — but which link actually
/// gets emitted is decided by integrated time
/// ([`crate::machine_kinematics::surface_link_time`] vs.
/// [`crate::machine_kinematics::retract_link_time`]), never by raw
/// distance/feed, whenever `params.link_kinematics` is `Some`. The P0
/// unified-finishing probe (`planning/unified_finishing_pass_plan.md`) found
/// the naive distance/feed estimate misjudges wall-clock by up to 10× on
/// segmented paths (3D Finish 6: 10.1× naive) — junction/accel physics, not
/// commanded feed, dominates once segments get short, so a distance-only
/// hookup heuristic picks the wrong link on exactly the paths where it
/// matters most. `params.link_kinematics = None` keeps the legacy behaviour
/// (surface link whenever `build_surface_link` succeeds within
/// `hookup_distance`).
///
/// Each `PencilPath` is itself split at non-contact (NaN-Z) points via
/// [`contact_runs`] before emission, so an off-mesh gap in the middle of a
/// pass produces two independent runs — each with its own rapid/plunge or
/// surface link — rather than a single cutting move bridging the gap.
///
/// Entries have no stock reading on this form, so they keep the legacy single
/// fed descent. Every production caller — the pencil generator and
/// [`crate::unified_finish`]'s pencil-claims pipeline — now holds the input
/// stock and calls [`emit_paths_with_entry_stock`] directly (G-ENTRYLOAD), so
/// this wrapper survives only as the tests' stock-less spelling.
#[cfg(test)]
pub(crate) fn emit_paths(
    all_paths: &[PencilPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    emit_paths_with_entry_stock(all_paths, mesh, index, cutter, params, None)
}

/// [`emit_paths`] with the input stock the entries are descending through.
///
/// `entry_stock` is only ever read to plan the entry manoeuvre
/// ([`plan_entry_ramp`]); `None` reproduces the pre-G-ENTRYLOAD emission
/// exactly, which is what makes the A/B in
/// `tests/pencil_entry_ramp_g_entryload.rs` a controlled one.
pub(crate) fn emit_paths_with_entry_stock(
    all_paths: &[PencilPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    entry_stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    use crate::toolpath::MoveIntent;

    let contact_radius = tip_contact_radius(cutter);
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();
    let mut prev_end: Option<P3> = None;

    for path in all_paths {
        if path.points.len() < 2 {
            continue;
        }

        for run in contact_runs(&path.points) {
            if run.len() < 2 {
                continue;
            }

            let move_index = tp.moves.len();
            let first = *run.first().unwrap_or(&P3::origin());
            // Where the body pass starts. A ramped entry has already cut the
            // window at its finished Z, so the body resumes past it.
            let mut body_start = 1usize;

            // Try to link from the previous run's end without retracting.
            // `hookup_distance` is only the candidate CAP (gap must be within
            // it, and the link must be gouge-safe via `build_surface_link`);
            // when the caller supplied `link_kinematics`, the candidate is
            // additionally costed against a retract-link candidate with the
            // F-034 integrator, and only kept when it's actually cheaper —
            // see the doc comment above.
            let link = prev_end.and_then(|end| {
                let gap = ((first.x - end.x).powi(2) + (first.y - end.y).powi(2)).sqrt();
                if gap <= 1e-6 || gap > params.hookup_distance {
                    return None;
                }
                let link_pts = build_surface_link(
                    end,
                    first,
                    mesh,
                    index,
                    cutter,
                    params.stock_to_leave,
                    params.sampling,
                )?;
                match &params.link_kinematics {
                    Some(lk) => {
                        let mut costed_path = link_pts.clone();
                        costed_path.push(first);
                        let surface_t = crate::machine_kinematics::surface_link_time(
                            end,
                            &costed_path,
                            params.feed_rate,
                            &lk.kinematics,
                            lk.max_feed_mm_min,
                            lk.rapid_feed_mm_min,
                        );
                        // The retract candidate's descent isn't split by a
                        // rapid-down-to-clearance: the cost model can't
                        // verify the input stock's ceiling from here (that
                        // check lives in the post-generation
                        // `optimize_entry_descents` pass), so it must not
                        // assume a descent it can't guarantee is safe.
                        let retract_t = crate::machine_kinematics::retract_link_time(
                            end,
                            first,
                            params.safe_z,
                            None,
                            params.plunge_rate,
                            &lk.kinematics,
                            lk.max_feed_mm_min,
                            lk.rapid_feed_mm_min,
                        );
                        (surface_t <= retract_t).then_some(link_pts)
                    }
                    None => Some(link_pts),
                }
            });

            match link {
                Some(link_pts) => {
                    // Surface-following link (no retract / no re-plunge), then the body.
                    for lp in &link_pts {
                        tp.feed_to_with_intent(*lp, params.feed_rate, MoveIntent::Linking);
                    }
                    tp.feed_to_with_intent(first, params.feed_rate, MoveIntent::Linking);
                }
                None => {
                    // Too far (or unsafe) to link: retract the previous run, then a
                    // fresh rapid-over + entry.
                    if let Some(end) = prev_end {
                        tp.rapid_to_with_intent(
                            P3::new(end.x, end.y, params.safe_z),
                            MoveIntent::Retract,
                        );
                    }
                    tp.rapid_to_with_intent(
                        P3::new(first.x, first.y, params.safe_z),
                        MoveIntent::Linking,
                    );
                    match entry_stock
                        .and_then(|stock| plan_entry_ramp(run, stock, contact_radius, params))
                    {
                        Some(plan) => {
                            // Air only — the descent stops at the conservative
                            // stock ceiling, so the plunge-rate cap is paid on
                            // nothing but clearance (and `dressup::
                            // optimize_entry_descents` turns most of even that
                            // into a rapid).
                            if plan.air_descent_z < params.safe_z - 1e-9 {
                                tp.feed_to_with_intent(
                                    P3::new(first.x, first.y, plan.air_descent_z),
                                    params.plunge_rate,
                                    MoveIntent::EntryPlunge,
                                );
                            }
                            for p in &plan.points {
                                tp.feed_to_with_intent(*p, params.feed_rate, MoveIntent::EntryRamp);
                            }
                            tracing::debug!(
                                window_end = plan.window_end,
                                step_mm = plan.step_mm,
                                ramp_points = plan.points.len(),
                                "pencil entry ramped along the crease"
                            );
                            body_start = plan.window_end + 1;
                        }
                        None => {
                            tp.feed_to_with_intent(
                                first,
                                params.plunge_rate,
                                MoveIntent::EntryPlunge,
                            );
                        }
                    }
                }
            }

            // Feed the body (skipping whatever the entry already cut — at
            // minimum the first point, which we are already standing on).
            for p in run.iter().skip(body_start) {
                tp.feed_to_with_intent(*p, params.feed_rate, MoveIntent::FinishingCut);
            }
            prev_end = run.last().copied();

            annotations.push(PencilRuntimeAnnotation {
                move_index,
                event: PencilRuntimeEvent::OffsetPass {
                    chain_index: path.chain_index,
                    chain_total: path.chain_total,
                    offset_index: path.offset_index,
                    offset_total: path.offset_total,
                    offset_mm: path.offset_mm,
                    is_centerline: path.is_centerline,
                },
            });
        }
    }

    // Final retract once everything is emitted.
    if let Some(end) = prev_end {
        tp.rapid_to_with_intent(P3::new(end.x, end.y, params.safe_z), MoveIntent::Retract);
    }

    (tp, annotations)
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used, clippy::too_many_arguments)]
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
    // Out: the RestDepth detector's rest-field grid, for the GUI heatmap
    // overlay. Set only when `detector == RestDepth`; left untouched otherwise.
    rest_grid_out: &mut Option<crate::rest_field::RestGrid>,
    // Out: the RestDepth detector's derived machining-region polygons (P2.2
    // selective-finishing boundary source). Set only when
    // `detector == RestDepth`; left untouched otherwise.
    rest_regions_out: &mut Option<Vec<Polygon2>>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    let never_cancel = || false;
    pencil_toolpath_structured_annotated_with_cancel(
        mesh,
        index,
        cutter,
        params,
        initial_stock,
        debug,
        rest_grid_out,
        rest_regions_out,
        // Wave D1: this legacy entry point predates the tip-float channel
        // and has no slot to return it through. Callers that need the
        // finding (the op adapter does) call the cancellable form.
        &mut None,
        &never_cancel,
    )
    .expect("non-cancellable pencil toolpath should never be cancelled")
}

/// Diameter (mm) of the vanishingly small "surface probe" ball substituted
/// when there's no real or nominal reference tool to compare against (the
/// self-referenced-gap case, [`ResolvedReference::SelfReferenced`] /
/// [`crate::rest_field::RestReference`]'s `is_surface_probe` arm). Small
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
enum ResolvedReference<'a> {
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
    /// [`RestReference`]: crate::rest_field::RestReference
    SelfReferenced,
}

impl ResolvedReference<'_> {
    /// The `Option<&dyn MillingCutter>` shape the Dihedral/Curvature gates
    /// want: `None` means self-referenced gap.
    fn as_dyn(&self) -> Option<&dyn MillingCutter> {
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
/// The comparison is now against `cusp_radius_mm() * 2` — the tip diameter,
/// which is `diameter()` for every non-tapered shape, so nothing but the
/// tapered path moves.
fn resolve_reference_cutter<'a>(
    params: &'a PencilParams,
    pencil: &dyn MillingCutter,
) -> ResolvedReference<'a> {
    if let Some(rc) = params.reference_cutter.as_ref() {
        return ResolvedReference::Real(rc);
    }
    let pencil_cutting_diameter = pencil.cusp_radius_mm() * 2.0;
    if params.reference_tool_diameter > pencil_cutting_diameter + 1e-6 {
        return ResolvedReference::Nominal(crate::tool::BallEndmill::new(
            params.reference_tool_diameter,
            NOMINAL_REFERENCE_BALL_LENGTH_MM,
        ));
    }
    ResolvedReference::SelfReferenced
}

/// `Curvature` detector arm (see [`crate::crest_lines`]): trace the zero-set
/// of the minimal-curvature extremality, filtered by the single
/// `valley_saliency` (|κ₂|) dial. Keep lines long enough, then optionally
/// apply the reference-tool rest-depth gate (only when `min_valley_depth >
/// 0`, so saliency alone can drive selection), resample to cut spacing, and
/// build paths. No bisector — the crest line already sits on the valley
/// floor. Polls `cancel` once per line, matching the other two arms.
fn curvature_arm(
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

    let cp = crate::crest_lines::CrestParams {
        valley_saliency: params.valley_saliency,
        smoothing_iters: params.curvature_smoothing,
        min_line_length: params.min_cut_length,
    };
    let lines = crate::crest_lines::detect_valley_lines(mesh, &cp);
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

/// `RestDepth` detector arm (see [`crate::rest_field`]): build the dual-tool
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
fn rest_depth_arm(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    debug: Option<&ToolpathDebugContext>,
    rest_grid_out: &mut Option<crate::rest_field::RestGrid>,
    rest_regions_out: &mut Option<Vec<Polygon2>>,
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
        crate::rest_field::RestReference<'_>,
        bool,
        u8,
    ) = if let Some(stock) = stock_ref {
        (crate::rest_field::RestReference::Stock(stock), false, 2)
    } else {
        match &resolved {
            ResolvedReference::Real(r) => (
                crate::rest_field::RestReference::Cutter {
                    tool: *r,
                    is_surface_probe: false,
                },
                false,
                1,
            ),
            ResolvedReference::Nominal(b) => (
                crate::rest_field::RestReference::Cutter {
                    tool: b as &dyn MillingCutter,
                    is_surface_probe: false,
                },
                false,
                0,
            ),
            ResolvedReference::SelfReferenced => (
                crate::rest_field::RestReference::Cutter {
                    tool: &probe_ball as &dyn MillingCutter,
                    is_surface_probe: true,
                },
                true,
                0,
            ),
        }
    };
    let rf_params = crate::rest_field::RestFieldParams {
        cell_mm: params.rest_cell_mm,
        min_valley_depth: params.min_valley_depth,
        // Coverage routing (PR-5): the detector routes against the fan this
        // arm is about to emit, so it is handed the SAME two numbers
        // `centerline_cut_paths` gets below. `route_width_factor` is no
        // longer read — it is deprecated and reported, see
        // `PencilParams::route_width_factor`.
        offset_stepover_mm: params.offset_stepover,
        num_offset_passes_cap: params.num_offset_passes,
        min_cut_length: params.min_cut_length,
        // No dedicated PencilParams dial yet — the default margin (P2.1
        // scope: derive region_polygons, not expose a new user-facing knob).
        ..Default::default()
    };
    let rf = crate::rest_field::detect_rest_valleys(mesh, index, cutter, reference, &rf_params);
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
    *rest_grid_out = Some(rf.rest_grid);
    // Same for the derived machining-region polygons (P2.2 selective-finishing
    // boundary source) — set alongside the grid, independent of whether any
    // centreline survives the length gate.
    *rest_regions_out = Some(rf.region_polygons);
    // Length-gate + resample + width-capped offset-pass emission, factored
    // into `crease_paths::centerline_cut_paths` so the P2 finish planner's
    // future crease pass can reuse it without pencil's detector dispatch.
    let all_paths = crate::crease_paths::centerline_cut_paths(
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

/// `Dihedral` detector arm (see [`crate::pencil_dihedral`]): the historical
/// mesh-crease detector. Angle-filter candidate edges, chain them, keep only
/// chains holding genuine REST material (`rest_depth = reference_gap −
/// pencil_gap > threshold`), then sample each surviving chain
/// bisector-positioned into cut-spaced paths.
fn dihedral_arm(
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

/// Cancellable variant of [`pencil_toolpath_structured_annotated`]. Polls
/// `cancel` between the major phases (detector dispatch → path ordering →
/// emission) and once per line/chain inside each detector arm's own sampling
/// loop (see [`dihedral_arm`], [`curvature_arm`], [`rest_depth_arm`]). Does
/// NOT poll inside the detector internals themselves
/// (`crest_lines::detect_valley_lines`, `rest_field::detect_rest_valleys`,
/// `chain_concave_edges`/`gate_chains_by_depth`) — those stay as a follow-up
/// if they ever show up as the actual long pole.
#[allow(clippy::too_many_arguments)]
pub fn pencil_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    debug: Option<&ToolpathDebugContext>,
    rest_grid_out: &mut Option<crate::rest_field::RestGrid>,
    rest_regions_out: &mut Option<Vec<Polygon2>>,
    // Wave D1 out: the centreline TIP-FLOAT tally (see [`TipFloatFinding`]).
    // Always set — a detector that emitted no centreline still measured
    // zero points, which is a different statement from "not measured", and
    // the op adapter is what turns the distinction into a report.
    tip_float_out: &mut Option<TipFloatFinding>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<PencilRuntimeAnnotation>), Cancelled> {
    check_cancel(cancel)?;
    let tp = Toolpath::new();
    let annotations = Vec::new();
    let mut float = TipFloatFinding::default();

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
            rest_grid_out,
            rest_regions_out,
            &mut float,
            cancel,
        )?,
        PencilDetector::Dihedral => dihedral_arm(mesh, index, cutter, params, &mut float, cancel)?,
    };
    *tip_float_out = Some(float);

    if all_paths.is_empty() {
        return Ok((tp, annotations));
    }
    check_cancel(cancel)?;

    // Step 6: Order paths by nearest-neighbor
    order_paths_nearest(&mut all_paths);
    check_cancel(cancel)?;

    // Step 7: Emit toolpath. The input stock goes in so entries can ramp
    // along the crease instead of carving down into it (G-ENTRYLOAD).
    let (tp, annotations) =
        emit_paths_with_entry_stock(&all_paths, mesh, index, cutter, params, initial_stock);

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

    Ok((tp, annotations))
}

pub fn pencil_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations) = pencil_toolpath_structured_annotated(
        mesh, index, cutter, params, None, debug, &mut None, &mut None,
    );
    (tp, runtime_annotations_to_labels(&annotations))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};
    use crate::tool::BallEndmill;

    /// Create a flat-only mesh (convex-only, no concave edges). Also kept
    /// (duplicated) as a tiny same-module helper in `pencil_dihedral::tests`
    /// for its own concavity-detection unit tests.
    fn make_convex_box(size: f64) -> TriangleMesh {
        // Simple flat square — no concave edges possible with 2 triangles
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(size, 0.0, 0.0),
            P3::new(size, size, 0.0),
            P3::new(0.0, size, 0.0),
        ];
        let triangles = vec![[0, 1, 2], [0, 2, 3]];
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Coverage ported from `pencil_dihedral`'s retired `sample_chain` (the
    /// un-offset predecessor of `resample_polyline`): a straight 20mm
    /// segment sampled at 2mm spacing should yield ~11 points including both
    /// endpoints, all still exactly on the source line.
    #[test]
    fn test_resample_polyline_spacing() {
        let line = vec![P3::new(0.0, 0.0, -5.0), P3::new(20.0, 0.0, -5.0)];
        let points = resample_polyline(&line, 2.0);

        assert!(
            points.len() >= 8,
            "Should get at least 8 sample points on 20mm line at 2mm spacing, got {}",
            points.len()
        );
        for p in &points {
            assert!(
                (p.y - 0.0).abs() < 0.1,
                "Points should be at y=0, got y={}",
                p.y
            );
            assert!(
                (p.z - (-5.0)).abs() < 0.1,
                "Points should be at z=-5, got z={}",
                p.z
            );
        }
        let first = points.first().unwrap();
        let last = points.last().unwrap();
        assert!((first.x - 0.0).abs() < 1e-9, "should preserve first vertex");
        assert!((last.x - 20.0).abs() < 1e-9, "should preserve last vertex");
    }

    #[test]
    fn test_pencil_toolpath_v_groove() {
        let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 5.0,
            hookup_distance: 20.0,
            offset_stepover: 1.5,
            sampling: 1.0,
            ..Default::default()
        };

        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        assert!(
            !tp.moves.is_empty(),
            "V-groove should produce pencil toolpath moves"
        );
    }

    #[test]
    fn test_pencil_toolpath_convex_empty() {
        let mesh = make_convex_box(50.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = PencilParams {
            min_cut_length: 1.0,
            hookup_distance: 20.0,
            offset_stepover: 1.5,
            ..Default::default()
        };

        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        assert!(
            tp.moves.is_empty(),
            "Convex mesh should produce empty pencil toolpath"
        );
    }

    #[test]
    fn test_pencil_with_offset_passes() {
        let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params_center = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 5.0,
            hookup_distance: 20.0,
            offset_stepover: 1.5,
            sampling: 1.0,
            ..Default::default()
        };

        let params_offset = PencilParams {
            num_offset_passes: 2,
            // Set explicitly so `..params_center` never moves the non-Copy
            // `reference_cutter` / `link_kinematics`, keeping `params_center`
            // usable below.
            reference_cutter: None,
            link_kinematics: None,
            ..params_center
        };

        let tp_center = pencil_toolpath(&mesh, &index, &tool, &params_center);
        let tp_offset = pencil_toolpath(&mesh, &index, &tool, &params_offset);

        // Offset passes should produce more moves
        assert!(
            tp_offset.moves.len() > tp_center.moves.len(),
            "Offset passes ({}) should produce more moves than center-only ({})",
            tp_offset.moves.len(),
            tp_center.moves.len()
        );
    }

    /// Finely-tessellated gentle sine·sine surface. Wavelength (20mm) and
    /// amplitude (0.3mm) give a local concave radius of curvature ~30mm — far
    /// larger than a 1mm tool's radius, so a 1mm ball REACHES every trough.
    /// Correct tool-radius-aware pencil output for a 1mm tool here is ~empty.
    /// Today's raw-dihedral detector instead fires on every trough edge.
    fn make_gentle_undulating_surface(
        extent: f64,
        n: usize,
        lambda: f64,
        amp: f64,
    ) -> TriangleMesh {
        use std::f64::consts::PI;
        let mut vertices = Vec::with_capacity((n + 1) * (n + 1));
        let step = extent / n as f64;
        for j in 0..=n {
            for i in 0..=n {
                let x = i as f64 * step;
                let y = j as f64 * step;
                let z = amp * (2.0 * PI * x / lambda).sin() * (2.0 * PI * y / lambda).sin();
                vertices.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
        let mut triangles = Vec::with_capacity(n * n * 2);
        for j in 0..n {
            for i in 0..n {
                // CCW for upward (+Z) normals on a heightfield.
                triangles.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                triangles.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Count distinct pencil chains produced for a given tool, exercising the
    /// detection→gate→chaining path (the geometry the toolpath is built from),
    /// including the tool-radius-aware reach-gap gate.
    fn pencil_chain_count(
        mesh: &TriangleMesh,
        tool: &dyn MillingCutter,
        params: &PencilParams,
    ) -> usize {
        let index = SpatialIndex::build(mesh, 5.0);
        let edge_map = build_edge_adjacency(mesh);
        let shared = compute_shared_edges(mesh, &edge_map);
        let threshold_rad = params.bitangency_angle.to_radians();
        let concave: Vec<SharedEdge> = shared
            .into_iter()
            .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
            .collect();
        let chains_all = chain_concave_edges(&concave, mesh, params.min_cut_length);
        let resolved = resolve_reference_cutter(params, tool);
        let reference_ref = resolved.as_dyn();
        gate_chains_by_depth(
            chains_all,
            mesh,
            &index,
            tool,
            reference_ref,
            params.min_valley_depth,
        )
        .len()
    }

    fn default_pencil_params_1mm() -> PencilParams {
        PencilParams {
            hookup_distance: 3.0,
            offset_stepover: 0.25,
            feed_rate: 2000.0,
            plunge_rate: 132.0,
            ..Default::default()
        }
    }

    /// Tool-radius-aware detection acceptance. A 1mm tool on a gentle surface it
    /// can fully reach yields ~no pencil chains (the reach-gap gate rejects the
    /// reachable troughs); a genuine sharp valley it cannot bottom still yields a
    /// chain. The valley uses a correctly-wound heightfield (`make_v_valley`) —
    /// `make_v_groove`'s walls face downward, which `drop_cutter` rightly skips.
    #[test]
    fn test_pencil_tool_radius_aware_gentle_vs_valley() {
        let tool = BallEndmill::new(1.0, 25.0); // 1mm ball ≈ the detail tool's tip

        let gentle = make_gentle_undulating_surface(40.0, 80, 20.0, 0.3);
        let gentle_chains = pencil_chain_count(&gentle, &tool, &default_pencil_params_1mm());

        // Deep narrow valley (slope 2.0 → ~3mm rise per 1.5mm): a 1mm tool bridges it.
        let valley = make_v_valley(20.0, 4.0, 2.0, 20, 32);
        let valley_chains = pencil_chain_count(&valley, &tool, &default_pencil_params_1mm());

        assert!(
            valley_chains >= 1,
            "sharp V-valley must yield ≥1 pencil chain for a 1mm tool, got {valley_chains}"
        );
        assert!(
            gentle_chains <= 1,
            "gentle reachable surface should yield ~0 pencil chains for a 1mm tool \
             once the reach-gap gate is applied, got {gentle_chains}"
        );
    }

    /// Opt-in real-mesh validation against the organic relief that exploded:
    ///   RS_CAM_PENCIL_FIXTURE=/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl \
    ///   cargo test -p rs_cam_core --lib pencil_real_mesh_gate -- --ignored
    /// Asserts the reach-gap gate reduces the candidate chain set and still traces
    /// genuine valleys. Observed 2026-06-25 (terrain.stl, 661,212 tris, 1mm ball,
    /// bitangency 160, gap 0.05mm): angle_chains=9277 → gated_chains=6994,
    /// gated_moves=64,256 (vs 366k pre-gate), cutting=81m, rapid=77m. The gate is
    /// correct but modest here (the surface is tool-unreachable almost everywhere);
    /// the rapids are the Stage-2 hookup target.
    #[test]
    #[ignore = "needs RS_CAM_PENCIL_FIXTURE=/path/to/terrain.stl"]
    fn pencil_real_mesh_gate() {
        let path = std::env::var("RS_CAM_PENCIL_FIXTURE").unwrap();
        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(1.0, 25.0);
        let params = PencilParams {
            hookup_distance: 3.0,
            offset_stepover: 0.25,
            feed_rate: 2000.0,
            plunge_rate: 132.0,
            safe_z: mesh.bbox.max.z + 5.0,
            ..Default::default()
        };

        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);
        let thr = params.bitangency_angle.to_radians();
        let angle_owned: Vec<SharedEdge> = shared
            .into_iter()
            .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - thr))
            .collect();
        let angle_chains = chain_concave_edges(&angle_owned, &mesh, params.min_cut_length).len();
        let gated_chains = pencil_chain_count(&mesh, &tool, &params);
        let tp = pencil_toolpath(&mesh, &index, &tool, &params);

        assert!(
            gated_chains <= angle_chains,
            "reach-gap gate must not increase chains: gated={gated_chains} angle={angle_chains}"
        );
        assert!(
            !tp.moves.is_empty(),
            "pencil must still trace genuine valleys after gating"
        );
    }

    /// Minimal top-down 2D line raster of a toolpath (feed moves only) — fast and
    /// clear for seeing crease structure, unlike the 3D tube composite.
    #[allow(clippy::indexing_slicing)] // bounded by moves.len()
    fn rasterize_topdown(
        tp: &Toolpath,
        w: u32,
        h: u32,
        terrain_zmin: f64,
        terrain_zmax: f64,
    ) -> image::RgbaImage {
        use crate::toolpath::MoveType;
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([26, 26, 46, 255]));
        if tp.moves.len() < 2 {
            return img;
        }
        let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for m in &tp.moves {
            minx = minx.min(m.target.x);
            maxx = maxx.max(m.target.x);
            miny = miny.min(m.target.y);
            maxy = maxy.max(m.target.y);
        }
        // Colour is absolute to the TERRAIN height: floor = blue, top = green.
        let minz = terrain_zmin;
        let zrange = (terrain_zmax - terrain_zmin).max(1e-6);
        let margin = 20.0;
        let dw = (maxx - minx).max(1e-6);
        let dh = (maxy - miny).max(1e-6);
        let scale = ((w as f64 - 2.0 * margin) / dw).min((h as f64 - 2.0 * margin) / dh);
        for i in 1..tp.moves.len() {
            let feed = matches!(
                tp.moves[i].move_type,
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            );
            if !feed {
                continue;
            }
            let a = tp.moves[i - 1].target;
            let b = tp.moves[i].target;
            // Colour by height: deep (low Z, lake floors / valley bottoms) = blue,
            // high (top rims / ridges) = bright green. Lets us tell base from top.
            let zt = (((a.z + b.z) * 0.5 - minz) / zrange).clamp(0.0, 1.0);
            let col = image::Rgba([
                (40.0 + zt * 80.0) as u8,
                (90.0 + zt * 150.0) as u8,
                (210.0 - zt * 110.0) as u8,
                255,
            ]);
            let x1 = margin + (a.x - minx) * scale;
            let y1 = h as f64 - margin - (a.y - miny) * scale;
            let x2 = margin + (b.x - minx) * scale;
            let y2 = h as f64 - margin - (b.y - miny) * scale;
            let steps = (x2 - x1).abs().max((y2 - y1).abs()).ceil().max(1.0) as i32;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let px = x1 + (x2 - x1) * t;
                let py = y1 + (y2 - y1) * t;
                if px >= 0.0 && py >= 0.0 {
                    let (ux, uy) = (px as u32, py as u32);
                    if ux < w && uy < h {
                        img.put_pixel(ux, uy, col);
                    }
                }
            }
        }
        img
    }

    /// Fast headless visual loop (no GUI). Renders the pencil toolpath on a real
    /// mesh to a top-down PNG (and a browser-openable SVG). Params come from env
    /// vars so you can sweep WITHOUT recompiling — just re-run with different
    /// values:
    ///   RS_CAM_PENCIL_FIXTURE=.../terrain.stl RS_CAM_PENCIL_OUT=/tmp/p.png \
    ///   RS_CAM_PENCIL_MVD=0.2 RS_CAM_PENCIL_BIT=160 \
    ///   cargo test -p rs_cam_core --lib render_pencil_real_mesh -- --ignored
    #[test]
    #[ignore = "needs RS_CAM_PENCIL_FIXTURE; writes PNG to RS_CAM_PENCIL_OUT"]
    fn render_pencil_real_mesh() {
        let env_f64 = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        use std::time::Instant;
        let path = std::env::var("RS_CAM_PENCIL_FIXTURE").unwrap();
        let out = std::env::var("RS_CAM_PENCIL_OUT").unwrap_or_else(|_| "/tmp/pencil.png".into());
        let t = Instant::now();
        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let t_load = t.elapsed().as_millis();
        let t = Instant::now();
        // Default to the real GUI path (build_auto); override cell for experiments.
        let cell = env_f64("RS_CAM_PENCIL_CELL", 0.0);
        let index = if cell > 0.0 {
            SpatialIndex::build(&mesh, cell)
        } else {
            SpatialIndex::build_auto(&mesh)
        };
        let t_index = t.elapsed().as_millis();
        let tool = BallEndmill::new(2.0, 25.0); // ~2mm-tip finish ball

        let mut params = default_pencil_params_1mm();
        params.bitangency_angle = env_f64("RS_CAM_PENCIL_BIT", 160.0);
        params.min_valley_depth = env_f64("RS_CAM_PENCIL_MVD", 0.05);
        params.min_cut_length = env_f64("RS_CAM_PENCIL_MINLEN", 2.0);
        params.hookup_distance = env_f64("RS_CAM_PENCIL_HOOKUP", params.hookup_distance);
        params.bisector_strength = env_f64("RS_CAM_PENCIL_BISECTOR", params.bisector_strength);
        params.reference_tool_diameter =
            env_f64("RS_CAM_PENCIL_REFD", params.reference_tool_diameter);
        // Detector selection + curvature tuning (RS_CAM_PENCIL_DETECTOR=curvature).
        params.detector = match std::env::var("RS_CAM_PENCIL_DETECTOR").ok().as_deref() {
            Some(s) => PencilDetector::parse(s),
            None => PencilDetector::Dihedral,
        };
        params.valley_saliency = env_f64("RS_CAM_PENCIL_SAL", params.valley_saliency);
        params.curvature_smoothing =
            env_f64("RS_CAM_PENCIL_SMOOTH", params.curvature_smoothing as f64) as usize;
        params.safe_z = mesh.bbox.max.z + 5.0;

        // Phase timings for the detection sub-steps (the suspected hot path).
        let t = Instant::now();
        let em = build_edge_adjacency(&mesh);
        let t_adj = t.elapsed().as_millis();
        let t = Instant::now();
        let _sh = compute_shared_edges(&mesh, &em);
        let t_shared = t.elapsed().as_millis();

        let t = Instant::now();
        let tp = pencil_toolpath(&mesh, &index, &tool, &params);
        let t_gen = t.elapsed().as_millis();
        let _ = std::fs::write(
            out.replace(".png", ".timing.txt"),
            format!(
                "load_ms={t_load} index_ms={t_index} edge_adj_ms={t_adj} shared_edges_ms={t_shared} full_generate_ms={t_gen} (gate+chain+lift≈{})\n",
                t_gen.saturating_sub(t_adj + t_shared)
            ),
        );
        let moves = tp.moves.len();
        let cutting = tp.total_cutting_distance();
        let rapid = tp.total_rapid_distance();
        let _ = std::fs::write(
            out.replace(".png", ".stats.txt"),
            format!("moves={moves} cutting_mm={cutting:.0} rapid_mm={rapid:.0}\n"),
        );
        // Top-down 2D PNG (agent-readable) + SVG (browser-openable, crisp lines).
        let (w, h) = (1600u32, 1600u32);
        rasterize_topdown(&tp, w, h, mesh.bbox.min.z, mesh.bbox.max.z)
            .save(&out)
            .unwrap();
        let svg = crate::viz::toolpath_to_svg(&tp, w as f64, h as f64);
        std::fs::write(out.replace(".png", ".svg"), svg).unwrap();
        assert!(
            moves > 0,
            "rendered {out} | mvd={} bit={} moves={moves} cutting_mm={cutting:.0}",
            params.min_valley_depth,
            params.bitangency_angle,
        );
    }

    /// Correctly-wound (CCW, +Z normals) V-valley heightfield: z = -slope·half_y at
    /// the y=0 seam rising to 0 at the ±half_y edges. Unlike `make_v_groove`, the
    /// wall facets face up, so a dropped cutter rests on them.
    fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = -slope * half_y + slope * y.abs();
                verts.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// Fairing straightens a facet-scale zig-zag, pins the endpoints, and keeps
    /// the point count (so chains don't shrink at their tips).
    #[test]
    fn test_fair_polyline_straightens_and_pins_ends() {
        // Saw-tooth in Y along +X — the kind of jag a raw mesh-edge chain makes.
        let pts: Vec<P3> = (0..11)
            .map(|i| P3::new(i as f64, if i % 2 == 0 { 0.0 } else { 1.0 }, 0.0))
            .collect();
        let faired = fair_polyline_xy(&pts, FAIRING_PASSES, FAIRING_STRENGTH);

        assert_eq!(faired.len(), pts.len(), "fairing must preserve point count");

        let pf = pts.first().unwrap();
        let pl = pts.last().unwrap();
        let ff = faired.first().unwrap();
        let fl = faired.last().unwrap();
        assert!(
            (ff.x - pf.x).abs() < 1e-12 && (ff.y - pf.y).abs() < 1e-12,
            "first endpoint must be pinned"
        );
        assert!(
            (fl.x - pl.x).abs() < 1e-12 && (fl.y - pl.y).abs() < 1e-12,
            "last endpoint must be pinned"
        );

        // Peak interior Y excursion should shrink after fairing.
        let excursion = |v: &[P3]| {
            v.iter()
                .skip(1)
                .take(v.len().saturating_sub(2))
                .map(|p| p.y)
                .fold(0.0_f64, f64::max)
        };
        assert!(
            excursion(&faired) < excursion(&pts),
            "fairing should reduce zig-zag excursion ({} !< {})",
            excursion(&faired),
            excursion(&pts)
        );
    }

    /// Wiring `hookup_distance` joins nearby passes with a surface feed instead of
    /// a retract-rapid-replunge, so the total rapid distance drops.
    #[test]
    fn test_hookup_linking_reduces_rapids() {
        let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);
        let mk = |hd: f64| PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 5.0,
            hookup_distance: hd,
            num_offset_passes: 2,
            offset_stepover: 1.5,
            sampling: 1.0,
            ..Default::default()
        };

        let unlinked = pencil_toolpath(&mesh, &index, &tool, &mk(0.0));
        let linked = pencil_toolpath(&mesh, &index, &tool, &mk(50.0));

        assert!(!linked.moves.is_empty(), "linked path must still cut");
        assert!(
            linked.total_rapid_distance() < unlinked.total_rapid_distance(),
            "hookup linking should reduce rapids: linked={:.1} unlinked={:.1}",
            linked.total_rapid_distance(),
            unlinked.total_rapid_distance()
        );
    }

    /// P1 W4a — shared geometry for the cost-decision tests below: two
    /// independent 2-point runs 8mm apart on one continuous flat mesh (so
    /// `build_surface_link` always succeeds geometrically — the surface vs.
    /// retract choice is purely a cost decision, never a surface-contact
    /// fallback). Both runs sit comfortably off the box's diagonal seam
    /// (y=25 vs. the seam at y=x for every x in the runs' range).
    fn cost_decision_fixture(feed_rate: f64) -> Toolpath {
        let mesh = make_convex_box(40.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(2.0, 25.0);

        let path_a = PencilPath {
            points: vec![P3::new(10.0, 25.0, 0.0), P3::new(12.0, 25.0, 0.0)],
            chain_index: 1,
            chain_total: 2,
            offset_index: 1,
            offset_total: 1,
            offset_mm: 0.0,
            is_centerline: true,
        };
        let path_b = PencilPath {
            points: vec![P3::new(20.0, 25.0, 0.0), P3::new(22.0, 25.0, 0.0)],
            chain_index: 2,
            chain_total: 2,
            offset_index: 1,
            offset_total: 1,
            offset_mm: 0.0,
            is_centerline: true,
        };

        // A modest, unremarkable machine — the point of both tests is the
        // FEED rate ratio, not exotic accel/kinematics behaviour.
        let kin = crate::machine_kinematics::MachineKinematics {
            acceleration_mm_s2: 300.0,
            ..crate::machine_kinematics::MachineKinematics::default()
        };
        let params = PencilParams {
            hookup_distance: 20.0,
            feed_rate,
            plunge_rate: 500.0,
            safe_z: 15.0,
            sampling: 1.0,
            link_kinematics: Some(crate::machine_kinematics::LinkKinematics {
                kinematics: kin,
                max_feed_mm_min: 6000.0,
                rapid_feed_mm_min: 5000.0,
            }),
            ..Default::default()
        };

        emit_paths(&[path_a, path_b], &mesh, &index, &tool, &params).0
    }

    /// A slow commanded feed makes the direct 8mm surface link expensive
    /// (it cruises the whole gap at that feed) while the retract loop's
    /// climb/travel/descend runs at fast rapids regardless — the F-034
    /// costed decision must prefer retract even though the gap is well
    /// within `hookup_distance`.
    #[test]
    fn cost_decision_prefers_retract_when_surface_link_is_slow() {
        use crate::toolpath::{MoveIntent, MoveType};

        let tp = cost_decision_fixture(100.0);
        let cut_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.intent == MoveIntent::FinishingCut)
            .map(|(i, _)| i)
            .collect();
        let [cut_a, cut_b] = cut_indices.as_slice() else {
            panic!("expected exactly 2 FinishingCut moves (one per run), got {cut_indices:?}");
        };
        let between = tp.moves.get(*cut_a + 1..*cut_b).unwrap();
        assert!(
            between.iter().any(|m| m.move_type == MoveType::Rapid),
            "a slow surface feed should lose to the fast-rapid retract loop \
             — expected a rapid link between the two bodies, moves: {between:?}"
        );
    }

    /// A fast commanded feed makes the direct 8mm surface link cheap while
    /// the retract loop still pays its fixed climb/travel/descend distance
    /// regardless of rapid speed — the costed decision must prefer the
    /// surface link, so no rapid appears between the two bodies.
    #[test]
    fn cost_decision_prefers_surface_link_when_cheap() {
        use crate::toolpath::{MoveIntent, MoveType};

        let tp = cost_decision_fixture(3000.0);
        let cut_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.intent == MoveIntent::FinishingCut)
            .map(|(i, _)| i)
            .collect();
        let [cut_a, cut_b] = cut_indices.as_slice() else {
            panic!("expected exactly 2 FinishingCut moves (one per run), got {cut_indices:?}");
        };
        let between = tp.moves.get(*cut_a + 1..*cut_b).unwrap();
        assert!(
            !between.iter().any(|m| m.move_type == MoveType::Rapid),
            "a fast surface feed should beat the retract loop's fixed extra \
             distance — expected no rapid between the two bodies, moves: {between:?}"
        );
        assert!(
            between.iter().any(|m| m.intent == MoveIntent::Linking),
            "the winning surface link should emit Linking-intent feed moves"
        );
    }

    #[test]
    fn test_hemisphere_pencil_produces_ring() {
        // Hemisphere on a flat base has a concave ring where it meets the base
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 3.0,
            hookup_distance: 20.0,
            offset_stepover: 1.5,
            sampling: 1.0,
            safe_z: 25.0,
            ..Default::default()
        };

        let edge_map = build_edge_adjacency(&mesh);
        let shared = compute_shared_edges(&mesh, &edge_map);

        // Hemisphere should have concave edges where the dome meets steeper regions
        let _concave_count = shared.iter().filter(|e| e.is_concave).count();
        // The hemisphere is all convex from outside, but some edges at base may be concave
        // depending on tessellation. At minimum, the algorithm should not crash.
        let _tp = pencil_toolpath(&mesh, &index, &tool, &params);
        // We just verify it runs without panic — hemisphere may or may not produce edges
        // depending on tessellation quality
        assert!(
            !shared.is_empty(),
            "hemisphere tessellation should produce shared edges"
        );
    }

    /// Two disjoint flat squares along X with a gap between them (both at
    /// z=0) — used to force a genuinely isolated off-mesh point in the middle
    /// of a lifted polyline, with the gap wide enough that the tool's contact
    /// query radius (== cutter radius) never touches either square's material
    /// from the gap's centre.
    fn make_two_flat_squares(square_size: f64, gap: f64, half_width: f64) -> TriangleMesh {
        let vertices = vec![
            P3::new(0.0, -half_width, 0.0),
            P3::new(square_size, -half_width, 0.0),
            P3::new(square_size, half_width, 0.0),
            P3::new(0.0, half_width, 0.0),
            P3::new(square_size + gap, -half_width, 0.0),
            P3::new(2.0 * square_size + gap, -half_width, 0.0),
            P3::new(2.0 * square_size + gap, half_width, 0.0),
            P3::new(square_size + gap, half_width, 0.0),
        ];
        let triangles = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]];
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Regression: `lift_to_surface` marks an off-mesh point
    /// (bisector-shifted past the mesh edge) as non-contact via NaN-Z, and the
    /// emit loop (`emit_paths`) must split the pass there instead of a single
    /// cutting move bridging the gap with the stale un-lifted Z. Drives the
    /// real production path: `paths_from_sampled` (which calls
    /// `lift_to_surface`) feeding `emit_paths` (the extracted Step-7 logic).
    #[test]
    fn test_lift_to_surface_gap_splits_pass_not_stitches() {
        use crate::toolpath::MoveIntent;

        // Square A: x in [0,10]. Square B: x in [16,26]. Gap (10,16) has no
        // mesh at all, so a sample point near its centre (margin >= 3mm to
        // either edge, versus the 1mm-radius tool's query reach) is
        // unambiguously off-mesh.
        let mesh = make_two_flat_squares(10.0, 6.0, 5.0);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);

        // Raw (pre-lift) polyline: a dummy sentinel Z far from the real
        // surface (0.0) so an accidentally-un-lifted point is obvious. x=13.0
        // sits in the gap's centre.
        let xs = [0.0, 2.0, 4.0, 6.0, 8.0, 13.0, 18.0, 20.0, 22.0, 24.0, 26.0];
        let raw: Vec<P3> = xs.iter().map(|&x| P3::new(x, 0.0, 999.0)).collect();

        let run_with = |stock_to_leave: f64| {
            let params = PencilParams {
                bitangency_angle: 170.0,
                min_cut_length: 1.0,
                hookup_distance: 0.0, // force retract/replunge — isolate the split from linking
                offset_stepover: 1.5,
                sampling: 1.0,
                stock_to_leave,
                ..Default::default()
            };

            let mut all_paths = Vec::new();
            paths_from_sampled(
                &raw,
                1,
                1,
                &mesh,
                &index,
                &tool,
                params.stock_to_leave,
                params.offset_stepover,
                OffsetFan::symmetric(params.num_offset_passes),
                &mut all_paths,
                &mut TipFloatFinding::default(),
            );
            assert_eq!(all_paths.len(), 1, "centerline only, no offset passes");

            emit_paths(&all_paths, &mesh, &index, &tool, &params)
        };

        let (tp0, _annotations0) = run_with(0.0);
        assert!(
            !tp0.moves.is_empty(),
            "must still emit the two on-mesh runs"
        );

        // (a) no emitted move carries a non-finite Z — the NaN marker must
        // never leak past the split into an actual toolpath move.
        for m in &tp0.moves {
            assert!(
                m.target.z.is_finite(),
                "emitted move must never carry a non-finite Z, got {:?}",
                m.target
            );
        }

        // (b) the mid-path off-mesh gap must force two independent plunge
        // entries (one per on-mesh run) instead of one cutting move bridging
        // the gap with the previous pass's stale interpolated Z.
        let plunge_count = tp0
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::EntryPlunge)
            .count();
        assert_eq!(
            plunge_count, 2,
            "the mid-path off-mesh gap must split into two plunge entries, got {plunge_count}"
        );

        // No single cutting move should span anywhere near the ~6mm gap —
        // that would mean the two runs got stitched together instead of split.
        let mut prev: Option<P3> = None;
        for m in &tp0.moves {
            if m.intent == MoveIntent::FinishingCut
                && let Some(p) = prev
            {
                let dxy = ((m.target.x - p.x).powi(2) + (m.target.y - p.y).powi(2)).sqrt();
                assert!(
                    dxy < 4.0,
                    "a cutting move must not bridge the off-mesh gap: jumped {dxy:.2}mm"
                );
            }
            prev = Some(m.target);
        }

        // (c) stock_to_leave must be applied to every contacted (on-mesh) cut
        // Z — compare with/without.
        let (tp_leave, _annotations_leave) = run_with(0.5);
        let cut_zs = |tp: &Toolpath| -> Vec<f64> {
            tp.moves
                .iter()
                .filter(|m| matches!(m.intent, MoveIntent::FinishingCut | MoveIntent::EntryPlunge))
                .map(|m| m.target.z)
                .collect()
        };
        let zs0 = cut_zs(&tp0);
        let zs_leave = cut_zs(&tp_leave);
        assert_eq!(
            zs0.len(),
            zs_leave.len(),
            "stock_to_leave must not change which points are emitted as cuts"
        );
        for (z0, zl) in zs0.iter().zip(zs_leave.iter()) {
            assert!(
                (zl - z0 - 0.5).abs() < 1e-6,
                "stock_to_leave must shift every contacted cut Z by exactly 0.5mm: {z0} vs {zl}"
            );
        }
    }
}
