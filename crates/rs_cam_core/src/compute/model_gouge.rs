//! The model gouge guard (UX-7b, operator rulings 2026-10-22, SAFETY).
//!
//! A 2.5D op cuts a shape from a drawing or from the items of a model. On
//! a STEP or an STL model the op can cut into the part itself: a Pocket
//! that cuts every face ring to one floor, a Profile that cuts round a
//! ring down through a higher step, a Drill deeper than the blind hole of
//! the model. The guard reads the EMITTED cutting motion and the mesh of
//! the op's own model, and gives a [`ModelGougeFinding`]. The session
//! keeps the toolpath, so the operator can see it; the diagnostic is
//! blocking, and export refuses while the finding stands
//! (`ExportRefusal::CutsIntoModel`).
//!
//! The row states the rule ([`ModelGougeRule`]):
//!
//! - [`ModelGougeRule::Surface`] (Pocket, Profile, Zigzag, Adaptive 2D,
//!   Face): at each sample the tool tip must not go below the drop-cutter
//!   CL Z of the mesh at that XY.
//! - [`ModelGougeRule::Hole`] (Drill, Bore): a sample gouges only when the
//!   model has a hole at that XY (the point surface lies below the top
//!   round it) and the tip goes below the hole floor.
//! - [`ModelGougeRule::Bevel`] (Chamfer, R-C): the V-bit cuts below the
//!   sharp edge of the model by design, by the bevel depth `w / tan(a)`. A
//!   sample gouges when the tip goes deeper than the drop-cutter CL Z of
//!   the V-bit less that depth: the cone then cuts the model beyond the
//!   bevel (the end of an edge that runs into a wall).
//! - [`ModelGougeRule::Exempt`]: no guard.
//!
//! # The tangent wall
//!
//! A correct 2.5D cut leaves the tool TANGENT to a model wall (a Pocket at
//! exactly the tool radius from its wall; a 6 mm drill in a 6 mm hole). A
//! probe of the full radius touches the top edge of that wall and reads a
//! false gouge. The probe radius is the tool radius less
//! [`MODEL_GOUGE_TOL_MM`], so a tangent tool does not touch the wall, and a
//! tool that enters the wall by more than the tolerance does.
//!
//! # Sampling
//!
//! Each cutting move (every move that is not a rapid and not an obstacle
//! clearance) is sampled at a spacing of at most a quarter of the tool
//! diameter (the brief: "at most the tool radius / 2"), both ends included.
//! An arc samples along the arc (`geometry::arc_util::linearize_arc_into`).
//! The tip Z of a sample is the linear Z of the move at that point.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::compute::catalog::ModelGougeRule;
use crate::geo::P3;
use crate::interrupt::Cancelled;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::{FlatEndmill, MillingCutter};
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

/// The radial tolerance of the guard (mm): the CAD rounding of a drawn
/// length (`selection::CAD_ROUNDING_MM`, lead ruling L-13, 0.01 mm). A wall
/// within this distance of the tool is AT the tool (a tangent cut). The
/// coordinator brief asks for a value of about 0.01 to 0.05 mm with a
/// stated source; this is the one length in the repo that states "two
/// lengths closer than this are one value".
pub const MODEL_GOUGE_TOL_MM: f64 = crate::selection::CAD_ROUNDING_MM;

/// The Z tolerance of the guard (mm): a tip within this distance of the
/// surface is ON the surface. An STL flat region is a face at the Z of its
/// LOWEST triangle, and it holds triangles up to
/// `selection::FLAT_REGION_Z_BAND_MM` above that Z (L-7). A Pocket at "Each
/// picked face", depth 0, cuts to the lowest triangle, and the drop-cutter
/// reads the highest one under the tool. The tolerance is that band plus
/// the CAD rounding, so a cut to the face plane reads clean.
pub const MODEL_GOUGE_Z_TOL_MM: f64 =
    crate::selection::FLAT_REGION_Z_BAND_MM + crate::selection::CAD_ROUNDING_MM;

/// Samples between two cancel polls.
const CANCEL_POLL_SAMPLES: usize = 4096;

/// What the guard measured on one toolpath. `gouging == 0` is a measured
/// clean toolpath (the X-19 contract: an absent finding is "not measured").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelGougeFinding {
    /// The rule that the row states (never `Exempt` on a finding).
    pub rule: ModelGougeRule,
    /// The samples the guard judged: every sample over the mesh (a sample
    /// off the mesh footprint, or with no hole under the `Hole` rule, is
    /// not judged).
    pub judged: usize,
    /// The judged samples whose tip lies below the surface (or the hole
    /// floor) by more than [`MODEL_GOUGE_Z_TOL_MM`].
    pub gouging: usize,
    /// The deepest gouge (mm below the surface), `0.0` when clean.
    pub deepest_mm: f64,
    /// The tool tip at the deepest gouge, in the setup frame.
    pub at: [f64; 3],
    /// The surface Z (or the hole floor Z) under the deepest gouge.
    pub surface_z: f64,
    /// The index of the move that holds the deepest gouge.
    pub move_index: usize,
    /// F30: the Z that the depth dial of the op measures from (the stock
    /// box top of a Drill), from [`GougeProbe::depth_top_z`]. A Drill
    /// names the depth to type from it: `depth_top_z - surface_z`.
    pub depth_top_z: Option<f64>,
}

impl ModelGougeFinding {
    /// True when a sample cuts into the model.
    #[must_use]
    pub const fn gouges(&self) -> bool {
        self.gouging > 0
    }

    /// The operator sentence: the cause, where, and the fix of the row.
    #[must_use]
    pub fn message(&self) -> String {
        let [x, y, z] = self.at;
        if let ModelGougeRule::Bevel { fix } = self.rule {
            return format!(
                "The toolpath cuts into the model: {count} of {judged} sampled cutting \
                 positions cut deeper than the bevel. The deepest is {deep:.3} mm at X {x:.3} \
                 Y {y:.3}: the tool tip is at Z {z:.3}, and the bevel there may go down to Z \
                 {limit:.3}. {fix}. [Measured at generation on the emitted moves against the \
                 mesh of the toolpath model, tolerance {tol} mm in Z; export refuses while \
                 this stands.]",
                count = self.gouging,
                judged = self.judged,
                deep = self.deepest_mm,
                limit = self.surface_z,
                tol = MODEL_GOUGE_Z_TOL_MM,
            );
        }
        let fix = match (self.rule, self.depth_top_z) {
            // F30: the Drill names the depth to type, from the numbers the
            // guard measured.
            (
                ModelGougeRule::Hole {
                    names_depth: true, ..
                },
                Some(top),
            ) => format!(
                "Set the depth to {depth:.3} mm (the hole floor is at Z {floor:.3})",
                depth = top - self.surface_z,
                floor = self.surface_z,
            ),
            (rule, _) => rule.fix_hint().to_owned(),
        };
        format!(
            "The toolpath cuts into the model: {count} of {judged} sampled cutting positions \
             go below the model surface. The deepest is {deep:.3} mm at X {x:.3} Y {y:.3}: \
             the tool tip is at Z {z:.3} and the model surface is at Z {surface:.3}. {fix}. \
             [Measured at generation on the emitted moves against the mesh of the toolpath \
             model, tolerance {tol} mm in Z; export refuses while this stands.]",
            count = self.gouging,
            judged = self.judged,
            deep = self.deepest_mm,
            surface = self.surface_z,
            tol = MODEL_GOUGE_Z_TOL_MM,
        )
    }
}

/// The inputs of one measurement.
pub struct GougeProbe<'a> {
    /// The mesh of the op's own model, in the setup frame.
    pub mesh: &'a TriangleMesh,
    /// The spatial index of `mesh`.
    pub index: &'a SpatialIndex,
    /// The tool, with its radius less [`MODEL_GOUGE_TOL_MM`]
    /// ([`ModelGougeRule::Surface`]).
    pub reduced_tool: &'a dyn MillingCutter,
    /// The radius of the real tool (mm).
    pub tool_radius: f64,
    /// [`ModelGougeRule::Bevel`]: the bevel depth (mm) the cut may go
    /// below the V-bit CL Z (`OperationConfig::bevel_depth_mm`). `0.0` for
    /// every other rule.
    pub bevel_depth_mm: f64,
    /// F30: the Z the depth dial of the op measures from (the stock box
    /// top in the setup frame), for the fix text of a Drill. `None` when
    /// the caller does not know it.
    pub depth_top_z: Option<f64>,
}

/// Measure `toolpath` under `rule`. `None` for [`ModelGougeRule::Exempt`].
///
/// # Errors
///
/// [`Cancelled`] when `cancel` is set during the walk.
pub fn measure(
    toolpath: &Toolpath,
    rule: ModelGougeRule,
    probe: &GougeProbe<'_>,
    cancel: &AtomicBool,
) -> Result<Option<ModelGougeFinding>, Cancelled> {
    if matches!(rule, ModelGougeRule::Exempt) {
        return Ok(None);
    }
    // The point probe and the "round the hole" disc of the Hole rule. The
    // disc reaches just past a tangent wall: a drill of the hole size
    // reads the rim.
    let point = FlatEndmill::new(2.0 * MODEL_GOUGE_TOL_MM, 1.0);
    let disc = FlatEndmill::new(2.0 * (probe.tool_radius + 2.0 * MODEL_GOUGE_TOL_MM), 1.0);
    let spacing = (probe.tool_radius / 2.0).max(MODEL_GOUGE_TOL_MM);
    let mut finding = ModelGougeFinding {
        rule,
        judged: 0,
        gouging: 0,
        deepest_mm: 0.0,
        at: [0.0; 3],
        surface_z: 0.0,
        move_index: 0,
        depth_top_z: probe.depth_top_z,
    };
    let mut samples: Vec<P3> = Vec::new();
    let mut walked = 0_usize;
    let mut prev: Option<P3> = None;
    for (move_index, mv) in toolpath.moves.iter().enumerate() {
        let start = prev.replace(mv.target);
        let Some(start) = start else {
            continue;
        };
        if matches!(mv.move_type, MoveType::Rapid) || mv.intent == MoveIntent::ObstacleClearance {
            continue;
        }
        sample_move(start, mv.target, mv.move_type, spacing, &mut samples);
        for p in &samples {
            walked += 1;
            if walked.is_multiple_of(CANCEL_POLL_SAMPLES) && cancel.load(Ordering::Relaxed) {
                return Err(Cancelled);
            }
            let surface = match rule {
                ModelGougeRule::Surface { .. } => {
                    let cl =
                        point_drop_cutter(p.x, p.y, probe.mesh, probe.index, probe.reduced_tool);
                    cl.contacted.then_some(cl.z)
                }
                // R-C: the lowest tip Z the bevel may reach here.
                ModelGougeRule::Bevel { .. } => {
                    let cl =
                        point_drop_cutter(p.x, p.y, probe.mesh, probe.index, probe.reduced_tool);
                    cl.contacted.then_some(cl.z - probe.bevel_depth_mm)
                }
                ModelGougeRule::Hole { .. } => {
                    let floor = point_drop_cutter(p.x, p.y, probe.mesh, probe.index, &point);
                    let round = point_drop_cutter(p.x, p.y, probe.mesh, probe.index, &disc);
                    // A hole: the surface at the point lies below the top
                    // round it. A flat spot is the intent (a dowel hole).
                    (floor.contacted && round.contacted && round.z - floor.z > MODEL_GOUGE_Z_TOL_MM)
                        .then_some(floor.z)
                }
                ModelGougeRule::Exempt => None,
            };
            let Some(surface_z) = surface else {
                continue;
            };
            finding.judged += 1;
            let depth = surface_z - p.z;
            if depth > MODEL_GOUGE_Z_TOL_MM {
                finding.gouging += 1;
                if depth > finding.deepest_mm {
                    finding.deepest_mm = depth;
                    finding.at = [p.x, p.y, p.z];
                    finding.surface_z = surface_z;
                    finding.move_index = move_index;
                }
            }
        }
    }
    Ok(Some(finding))
}

/// The sample points of one cutting move from `start` to `end`, at most
/// `spacing` apart in XY, both ends included.
fn sample_move(start: P3, end: P3, move_type: MoveType, spacing: f64, out: &mut Vec<P3>) {
    match move_type {
        MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
            let clockwise = matches!(move_type, MoveType::ArcCW { .. });
            crate::geometry::arc_util::linearize_arc_into(
                out, start, end, i, j, clockwise, spacing,
            );
        }
        MoveType::Linear { .. } | MoveType::Rapid => {
            out.clear();
            let len = (end.x - start.x).hypot(end.y - start.y);
            // A move longer than `usize::MAX` samples is not a toolpath;
            // one step then judges its two ends.
            let steps = crate::util::steps::steps(len, spacing).unwrap_or(1).max(1);
            for k in 0..=steps {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "k and steps are sample counts far below 2^52"
                )]
                let t = k as f64 / steps as f64;
                out.push(P3::new(
                    start.x + t * (end.x - start.x),
                    start.y + t * (end.y - start.y),
                    start.z + t * (end.z - start.z),
                ));
            }
        }
    }
}
