//! The model gouge guard (UX-7b, operator rulings 2026-10-22, SAFETY).
//!
//! A 2.5D op cuts a shape from a drawing or from the items of a model. On
//! a STEP or an STL model the op can cut into the part itself: a Pocket
//! that cuts every face ring to one floor, a Profile that cuts round a
//! ring down through a higher step, a Drill deeper than the blind hole of
//! the model. The guard reads the EMITTED cutting motion and the mesh of
//! the op's own model, and gives a [`ModelGougeFinding`]. The session
//! keeps the toolpath, so the operator can see it; the diagnostic is
//! blocking in severity, and the export names it as a warning while the
//! finding stands (K6, ruling Q6: no finding refuses a job).
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
//! - [`ModelGougeRule::Allowance`] (3D Rough, G-ROUGHGOUGE): the op leaves
//!   stock on the model. A sample gouges when the tip goes below the
//!   drop-cutter CL Z plus [`GougeProbe::limit_offset_mm`] (the leave less
//!   the chord tolerance of the op). See "The allowance" below.
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
//! Each sample is one `point_drop_cutter` query through the spatial index
//! (two for `Hole`). A move gives its two ends plus one sample per D/4 of
//! XY length: a 3D Rough of 36 000 moves costs 72 000 queries plus its cut
//! length over D/4 (6 mm tool, 50 m of cut: about 105 000 queries).
//!
//! # The allowance (G-ROUGHGOUGE)
//!
//! The 3D Rough planner lifts each cut point to the `point_drop_cutter` CL
//! Z of the FULL tool plus the leave, at a spacing of at most the tool
//! radius, and then simplifies the cut by RDP at its chord tolerance
//! (`adaptive3d::clearing::stamp_emitted_segment`). So the promise of the
//! planner is: tip >= CL Z + leave - chord tolerance. The limit offset is
//! that leave less that tolerance, both read from the op config
//! (`OperationConfig::model_gouge_limit_offset_mm`), so the guard does not
//! report the simplifier as a gouge. The Z tolerance stays
//! [`MODEL_GOUGE_Z_TOL_MM`].
//!
//! The probe is the reduced tool of `Surface`. Its CL Z is at or below the
//! CL Z of the full tool, so a draped point never reads as a gouge.
//!
//! The radial tolerance (B2, operator ruling 2026-10-23: a default rough
//! must export; a graze within the op's own tolerance is not a defect).
//! A 2.5D op cuts its wall at the exact offset, so the CAD rounding
//! ([`MODEL_GOUGE_TOL_MM`]) is its only radial error. The 3D Rough plans
//! its path at its chord tolerance: the RDP step may move a draped chord
//! by that tolerance in 3D, so in XY too, and the tool side may come that
//! close to a wall by design (evidence run: 0.02 to 0.05 mm on the Corne
//! hex holes at a 0.3 mm slack). So under `Allowance` the probe radius is
//! the tool radius less the chord tolerance ([`probe_radial_tolerance_mm`]).
//! The 3D Rough has no radial leave (L2), so nothing else widens the probe.
//!
//! The class: a sample below the limit cuts into the leave. It cuts into
//! the model only when it is also below the model surface less the op
//! tolerance (`ModelGougeFinding::below_surface_mm`). Only that case is
//! Blocking and the export warns on it; a cut into the leave alone is Caution
//! ([`ModelGougeFinding::severity`]). Under `Allowance` the limit and that
//! surface differ by the leave at every sample, so the deepest sample below
//! the limit is also the deepest below the surface.
//!
//! The wall flag. A probe of radius `r` reads the highest material in its
//! disk, so a contact on the side of a wall reads the wall height as the
//! depth. The guard tests the deepest sample: the CL Z of any cutter of
//! radius `r` is at most the CL Z of a flat end mill of radius `r`, and
//! that is at most the surface under the centre plus `r * tan(a)`, with
//! `a` the steepest slope in the disk. So a CL Z more than `r` above the
//! surface under the tip centre (or no surface under the centre) proves a
//! slope steeper than 45 degrees in the disk: a wall. The message then says
//! that the depths are the height of that wall, and that the tool may touch
//! only its side. The Z-only measure cannot give the radial overlap.

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

/// G-ROUGHGOUGE: the two terms of the [`ModelGougeRule::Allowance`] limit,
/// from `OperationConfig::model_gouge_allowance`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModelGougeAllowance {
    /// The axial stock to leave of the op (mm).
    pub leave_mm: f64,
    /// The chord tolerance of the op (mm): the planner may put a chord this
    /// far below a draped point, and the tool side this far into a wall.
    pub tolerance_mm: f64,
}

impl ModelGougeAllowance {
    /// No allowance: every rule except `Allowance`.
    pub const NONE: Self = Self {
        leave_mm: 0.0,
        tolerance_mm: 0.0,
    };
}

/// The radial tolerance (mm) of the probe under `rule`: the probe radius is
/// `tool_radius` less it. [`MODEL_GOUGE_TOL_MM`] for every rule except
/// `Allowance`, which takes the chord tolerance of the op (B2, see the
/// module doc), kept between [`MODEL_GOUGE_TOL_MM`] and
/// `tool_radius - MODEL_GOUGE_TOL_MM` so the probe keeps a radius.
#[must_use]
pub fn probe_radial_tolerance_mm(
    rule: ModelGougeRule,
    allowance: ModelGougeAllowance,
    tool_radius: f64,
) -> f64 {
    match rule {
        ModelGougeRule::Allowance { .. } => {
            allowance_probe_radial_tolerance_mm(allowance, tool_radius)
        }
        ModelGougeRule::Surface { .. }
        | ModelGougeRule::Hole { .. }
        | ModelGougeRule::Bevel { .. }
        | ModelGougeRule::Exempt => MODEL_GOUGE_TOL_MM,
    }
}

/// The radial tolerance (mm) of the probe under [`ModelGougeRule::Allowance`]:
/// the chord tolerance of the op, kept between [`MODEL_GOUGE_TOL_MM`] and
/// `tool_radius - MODEL_GOUGE_TOL_MM`. ONE formula: the guard reads it
/// through [`probe_radial_tolerance_mm`], and the 3D Rough planner builds its
/// chord-lift probe with it (`Adaptive3dGeometry::gouge_probe`, G-DRAPEGAP).
#[must_use]
pub fn allowance_probe_radial_tolerance_mm(
    allowance: ModelGougeAllowance,
    tool_radius: f64,
) -> f64 {
    allowance
        .tolerance_mm
        .min(tool_radius - MODEL_GOUGE_TOL_MM)
        .max(MODEL_GOUGE_TOL_MM)
}

/// The reduced tool of the guard: `tool` less `radial_tol_mm` on its
/// radius ([`probe_radial_tolerance_mm`]). The corner radius of a bull nose
/// (or of a flat end mill) can be the full radius, so it shrinks with it
/// (the tool check holds a corner to half the diameter).
#[must_use]
pub fn reduced_tool_config(
    tool: &crate::compute::tool_config::ToolConfig,
    radial_tol_mm: f64,
) -> crate::compute::tool_config::ToolConfig {
    let mut reduced = tool.clone();
    reduced.diameter -= 2.0 * radial_tol_mm;
    reduced.corner_radius = reduced.corner_radius.min(reduced.diameter / 2.0);
    reduced.corner_radius_mm = reduced.corner_radius_mm.min(reduced.diameter / 2.0);
    reduced
}

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
    /// The surface Z (or the hole floor Z) under the deepest gouge. Under
    /// `Bevel` and `Allowance` it is the LIMIT Z: the lowest Z the tip may
    /// reach there (the CL Z less the bevel depth, or the CL Z plus the
    /// limit offset).
    pub surface_z: f64,
    /// The index of the move that holds the deepest gouge.
    pub move_index: usize,
    /// F30: the Z that the depth of the op measures from at the deepest
    /// gouge (F19: the rim of that hole of a Drill), from
    /// [`GougeProbe::depth_bases`]. A Drill names the depth to type from
    /// it: `depth_top_z - surface_z`.
    pub depth_top_z: Option<f64>,
    /// B2: the depth (mm) of the tip at `at` below the model surface less
    /// the op tolerance (`Allowance`: CL Z less the chord tolerance). Signed:
    /// negative when the tip stays above it (a cut into the leave only).
    /// Every other rule: equal to `deepest_mm`. `0.0` when clean. It sets
    /// the class ([`Self::severity`]).
    pub below_surface_mm: f64,
    /// B2 (`Allowance` only): at `at` a slope steeper than 45 degrees stands
    /// within the probe radius, or no surface lies under the tip centre (the
    /// wall test of the module doc). The depths are then the height of a
    /// wall above the tip.
    pub at_wall: bool,
}

impl ModelGougeFinding {
    /// True when a sample goes below the limit of the rule (for
    /// `Allowance`: into the leave, or deeper).
    #[must_use]
    pub const fn gouges(&self) -> bool {
        self.gouging > 0
    }

    /// True when the deepest sample cuts into the model itself: the export
    /// warns (`safety.cuts_into_model`, K6). Under every rule except
    /// `Allowance` this is [`Self::gouges`].
    #[must_use]
    pub fn blocks_export(&self) -> bool {
        self.gouges() && self.below_surface_mm > MODEL_GOUGE_Z_TOL_MM
    }

    /// The class of the finding: `None` when clean, `Blocking` when it cuts
    /// into the model ([`Self::blocks_export`]), and `Caution` when it cuts
    /// into the leave only (B2, operator ruling 2026-10-23).
    #[must_use]
    pub fn severity(&self) -> Option<crate::diagnostics::Severity> {
        use crate::diagnostics::Severity;
        if !self.gouges() {
            None
        } else if self.blocks_export() {
            Some(Severity::Blocking)
        } else {
            Some(Severity::Caution)
        }
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
                 mesh of the toolpath model, tolerance {tol} mm in Z; the export warns \
                 while this stands.]",
                count = self.gouging,
                judged = self.judged,
                deep = self.deepest_mm,
                limit = self.surface_z,
                tol = MODEL_GOUGE_Z_TOL_MM,
            );
        }
        if let ModelGougeRule::Allowance { fix } = self.rule {
            let blocks = self.blocks_export();
            let class = if blocks {
                "The toolpath cuts into the model"
            } else {
                "The toolpath cuts into the stock to leave, not into the model"
            };
            let surface = if self.below_surface_mm > MODEL_GOUGE_Z_TOL_MM {
                format!("{:.3} mm below", self.below_surface_mm)
            } else if self.below_surface_mm > 0.0 {
                format!("within {MODEL_GOUGE_Z_TOL_MM} mm of")
            } else {
                format!("{:.3} mm above", -self.below_surface_mm)
            };
            let wall = if self.at_wall {
                " A wall steeper than 45 degrees stands within the tool radius here, or no \
                 surface lies under the tool centre: the depths are the height of that wall \
                 above the tip, and the tool may touch only the side of the wall."
            } else {
                ""
            };
            let export = if blocks {
                "the export warns while this stands"
            } else {
                "the export does not warn on it"
            };
            return format!(
                "{class}: {count} of {judged} sampled cutting positions go below the limit of \
                 the stock to leave (the model surface plus the leave, less the op tolerance). \
                 The deepest is {deep:.3} mm below that limit at X {x:.3} Y {y:.3}: the tool \
                 tip is at Z {z:.3}, and the limit there is Z {limit:.3}. The tip is \
                 {surface} the model surface less the op tolerance.{wall} {fix}. [Measured at \
                 generation on the emitted moves against the mesh of the toolpath model, \
                 tolerance {tol} mm in Z; {export}.]",
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
             model, tolerance {tol} mm in Z; the export warns while this stands.]",
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
    /// [`ModelGougeRule::Allowance`]: the leave and the chord tolerance of
    /// the op (`OperationConfig::model_gouge_allowance`). The limit is the
    /// CL Z plus the leave less the tolerance; the model surface less the
    /// tolerance is the CL Z less the tolerance.
    /// [`ModelGougeAllowance::NONE`] for every other rule.
    pub allowance: ModelGougeAllowance,
    /// F30 (after F19): the XY and the depth base of each hole of a Drill
    /// whose floor is a depth below the Top Z, in the setup frame
    /// (`compute::execute::drill_depth_bases`). The fix text reads the base
    /// of the hole at the deepest gouge. Empty when the caller does not
    /// know them.
    pub depth_bases: &'a [([f64; 2], f64)],
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
        depth_top_z: None,
        below_surface_mm: 0.0,
        at_wall: false,
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
                // G-ROUGHGOUGE: the lowest tip Z the leave allows here.
                ModelGougeRule::Allowance { .. } => {
                    let cl =
                        point_drop_cutter(p.x, p.y, probe.mesh, probe.index, probe.reduced_tool);
                    cl.contacted
                        .then_some(cl.z + probe.allowance.leave_mm - probe.allowance.tolerance_mm)
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
    // F30: the depth base of the hole at the deepest gouge.
    if finding.gouges() {
        let [x, y, _] = finding.at;
        finding.depth_top_z = probe
            .depth_bases
            .iter()
            .map(|([hx, hy], base)| ((hx - x).hypot(hy - y), *base))
            .filter(|(d, _)| *d <= probe.tool_radius)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, base)| base);
        // B2: the class and the wall flag at the deepest sample.
        finding.below_surface_mm = finding.deepest_mm;
        if matches!(rule, ModelGougeRule::Allowance { .. }) {
            // The limit and the surface less the tolerance differ by the
            // leave at every sample.
            finding.below_surface_mm = finding.deepest_mm - probe.allowance.leave_mm;
            let cl_z = finding.surface_z - probe.allowance.leave_mm + probe.allowance.tolerance_mm;
            let centre = point_drop_cutter(x, y, probe.mesh, probe.index, &point);
            finding.at_wall = !centre.contacted || cl_z - centre.z > probe.reduced_tool.radius();
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
