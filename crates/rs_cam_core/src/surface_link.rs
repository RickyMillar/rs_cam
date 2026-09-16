//! Gouge-safe, drop-cutter-sampled surface links between two cut points.
//!
//! Promoted out of `crate::pencil` (where it joins consecutive pencil passes
//! without a retract-and-replunge) so the P2 unified-finish planner's global
//! router can reuse it as one of the two link-cost candidates (surface link
//! vs. retract link) between regions — see
//! `planning/unified_finish_planner_design.md` step 4.

use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;
use crate::trace::toolpath_spans::{AnnotatedToolpath, MoveRemap};
use crate::trace::transform_provenance::Transformed;

/// Build a gouge-safe, surface-following link between two cut points whose XY gap
/// is within `hookup_distance`, so consecutive passes join without retracting to
/// safe Z and re-plunging. Samples the connecting segment and drop-cutters each
/// interior point — the same gouge-free lift the cut path uses — so the tool rides
/// the surface across the gap instead of lifting clear. Endpoints are excluded
/// (the caller is already at `from` and feeds to `to` itself). Returns `None` if
/// the tool loses surface contact anywhere along the link (over a hole / off the
/// mesh) — the caller then falls back to a clean retract-and-replunge.
pub fn build_surface_link(
    from: P3,
    to: P3,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    spacing: f64,
) -> Option<Vec<P3>> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return Some(Vec::new());
    }
    let n = (dist / spacing.max(1e-3)).ceil().max(1.0) as usize;
    let mut pts = Vec::new();
    for k in 1..n {
        let t = k as f64 / n as f64;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        let cl = point_drop_cutter(x, y, mesh, index, cutter);
        if !cl.contacted {
            return None; // lost contact → not safe to link at surface, retract instead
        }
        pts.push(P3::new(x, y, cl.z + stock_to_leave));
    }
    Some(pts)
}

// ── Fragment relinking ───────────────────────────────────────────────────

/// Stock-aware ceiling for the points of a surface link.
///
/// A plain surface link rides the drop-cutter surface of the MESH, which is
/// only the whole truth when the mesh IS the material — true for a finishing
/// pass that has already had everything above it cleared, false for an
/// engraving pass run on raw or partly-roughed stock, where material stands
/// wherever nothing has cut yet. A link that rides the mesh there is a
/// cutting feed straight through whatever is standing above it.
///
/// When a caller supplies one of these, every link sample is lifted to
/// `max(mesh surface, material ceiling)` plus
/// [`crate::toolpath::PLUNGE_CLEARANCE_MM`],
/// the link is bracketed by a vertical exit and a vertical re-entry so the
/// traverse itself is entirely at that height, and the whole link is REFUSED
/// (the retract is kept) as soon as the ceiling reaches `safe_z` — at that
/// point the "link" would be a fed move at retract height, which is strictly
/// worse than the rapid it replaces.
///
/// `stock: None` inside a `Some(LinkCeiling)` is not the same as
/// `link_ceiling: None`: it means "no dexel snapshot in scope, use
/// [`Self::fallback_top_z`]" — the analytic fresh-stock top, the same
/// fallback [`crate::dressup::optimize_entry_descents`] takes. Only
/// `link_ceiling: None` disables the lift, and it is the byte-identical
/// legacy behaviour.
///
/// Since G-LINKSTAGE (2026-09-09) every opted-in finishing family takes a
/// ceiling through [`FinishingLinkStage`] whenever it holds an input stock
/// snapshot, and `None` means only "fresh stock, the mesh IS the material".
/// The scallop used to pass `None` unconditionally, which left its links
/// riding the mesh under whatever a prior op had left standing.
#[derive(Clone, Copy)]
pub struct LinkCeiling<'a> {
    /// The op's INPUT stock, when a simulated snapshot is in scope.
    pub stock: Option<&'a crate::dexel_stock::TriDexelStock>,
    /// SEARCH BOUND for the ceiling: the furthest lateral offset at which
    /// material could reach the tool at all — the tool's ENVELOPE radius.
    ///
    /// It is a bound, not the shape. Within it the cutter's own profile
    /// decides how high material has to stand before it can touch
    /// ([`Self::required_tip_z`]); only [`Self::material_top`] still reads the
    /// whole disc as a flat cylinder, and it is used where that cruder,
    /// strictly higher answer is the safe one.
    pub tool_radius: f64,
    /// Ceiling to assume where the dexel query has no answer (off-grid, or
    /// no snapshot at all). The analytic fresh-stock top.
    pub fallback_top_z: f64,
}

// Hand-written: `TriDexelStock` is not `Debug`, and printing a whole dexel
// grid into a relink log line would be useless anyway. Presence is the fact
// worth reporting.
impl std::fmt::Debug for LinkCeiling<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkCeiling")
            .field("stock", &self.stock.is_some())
            .field("tool_radius", &self.tool_radius)
            .field("fallback_top_z", &self.fallback_top_z)
            .finish()
    }
}

impl LinkCeiling<'_> {
    /// Highest Z at which material may stand anywhere under the tool at
    /// `(x, y)` — the raw dexel reading, over the whole disc.
    ///
    /// Exposed separately because a caller may need to ask *whether* anything
    /// stands above a candidate link before deciding to lift it.
    /// [`crate::pencil`] does: its links are between valley runs on a surface
    /// the finish pass has usually already cut to shape, so lifting every
    /// junction unconditionally would buy a
    /// [`crate::toolpath::PLUNGE_CLEARANCE_MM`] hop per junction and clear
    /// nothing. `relink_fragments`' own caller (engraving on raw stock) has
    /// the opposite prior and lifts unconditionally.
    ///
    /// This is the FLAT-CYLINDER reading: it asks only "how high does
    /// material stand under the tool disc", never "could the tool be there".
    /// It is therefore an upper bound on [`Self::required_tip_z`] and is kept
    /// deliberately for the two places where the cruder answer is the safe
    /// one — the flush-ride test below (a lower reading would ride the
    /// surface more often) and [`crate::pencil`]'s lift TRIGGER (a lower
    /// reading would lift less often).
    pub(crate) fn material_top(&self, x: f64, y: f64) -> f64 {
        self.stock
            .and_then(|s| s.max_conservative_top_z_in_disc(x, y, self.tool_radius))
            .unwrap_or(self.fallback_top_z)
    }

    /// Lowest tool TIP Z at `(x, y)` that nothing standing under the cutter
    /// can reach — the PROFILE-AWARE ceiling [`Self::clear_z`] is built on.
    ///
    /// [`Self::material_top`] models the cutter as a flat cylinder of
    /// [`Self::tool_radius`]. Past its tip a real cutter RISES, so material
    /// at lateral offset `r` can only strike it if it stands more than
    /// `height_at_radius(r)` above the tip:
    ///
    /// ```text
    /// tip_z >= max over r in [0, tool_radius] of
    ///            [ material_top_at(r) - height_at_radius(r) ]
    /// ```
    ///
    /// Measured on the operator's 200×200×9.81 mm relief with the shipped R1.0
    /// tapered ball (ball Ø2.0, 5.7° half-angle, Ø6 shank, envelope radius
    /// 3.0): the tool stands 10.97 mm above its tip at r = 2.0, above the
    /// board's ENTIRE relief, so only material within ~1.5 mm laterally can
    /// touch it — the flat disc over-reached by 2× in radius and lifted every
    /// link to the height of ridges that cannot contact the cutter. Holding
    /// the path identical and varying only this height moved one region from
    /// 898 s to 1411 s (1.57×).
    ///
    /// **Safety anchor:** for a FLAT endmill `height_at_radius` is `Some(0.0)`
    /// everywhere inside the envelope, so this is byte-identical to
    /// [`Self::material_top`]. The generalisation can only ever RELAX the lift
    /// where the tool genuinely rises above its own tip, and never for a flat
    /// cutter. See
    /// [`crate::dexel_stock::TriDexelStock::max_clearance_tip_z_for_profile`].
    pub(crate) fn required_tip_z(&self, cutter: &dyn MillingCutter, x: f64, y: f64) -> f64 {
        self.stock
            .and_then(|s| s.max_clearance_tip_z_for_profile(x, y, self.tool_radius, cutter))
            .unwrap_or(self.fallback_top_z)
    }

    /// Clearance height for a link sample at `(x, y)` whose mesh surface
    /// sits at `surface_z`. Use `f64::NEG_INFINITY` for `surface_z` when the
    /// drop cutter found no contact — the material ceiling then decides
    /// alone.
    ///
    /// The cutter is an ARGUMENT rather than a field so [`LinkCeiling`] stays
    /// `Copy`; every caller already holds the cutter it is planning for.
    pub(crate) fn clear_z(
        &self,
        cutter: &dyn MillingCutter,
        x: f64,
        y: f64,
        surface_z: f64,
    ) -> f64 {
        surface_z.max(self.required_tip_z(cutter, x, y)) + crate::toolpath::PLUNGE_CLEARANCE_MM
    }
}

/// What one fragment IS, so the stage knows whether it may change where the
/// fragment starts.
///
/// A fragment is a maximal run of non-`Rapid` moves. That says nothing about
/// its TOPOLOGY, and the difference decides whether the stage has one
/// candidate entry point or a whole circumference of them:
///
/// * [`Self::OpenRun`] — the ends are fixed. A raster row, a pencil trace, a
///   ring arc that the keep predicate split. The stage never reverses it
///   (that would flip climb/conventional), so its entry is its first point
///   and nothing else.
/// * [`Self::ClosedLoop`] — the fragment ends where it starts. A scallop
///   ring, a waterline loop, an iso-field level set. Every point on it is a
///   legal entry, so the stage may ROTATE the loop to begin at the point
///   nearest the previous fragment's exit, and close it there instead.
///
/// Rotation is what makes reordering pay on rings. Measured on the wanaka200
/// island scallop (`planning/linking_2026-09-09/SPEC.md` §7): of 600 ring
/// junctions the relink rejected 493 as `too_far` and ZERO on the kinematics
/// or surface tests, because the offset cascade emits rings breadth-first and
/// each run started at the offset library's own start vertex — two radially
/// adjacent rings 1.03 mm apart met at unrelated points of their
/// circumference. That is a candidate-set defect, not a distance-cap one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FragmentKind {
    /// Fixed ends. The default, and the byte-identical legacy treatment.
    #[default]
    OpenRun,
    /// Closed: the stage may rotate the loop to start near the previous exit.
    ClosedLoop,
}

/// The rate and tolerance half of a relink, which every generator already
/// holds in its own params.
///
/// Split from [`FinishingLinkStage`] so the shared finishing configuration
/// can be built once from an [`crate::compute`] execution context and handed
/// to a generator that supplies these from its own dials — the two halves
/// come from different places and neither can be derived from the other.
#[derive(Debug, Clone, Copy)]
pub struct LinkGeometry {
    pub stock_to_leave: f64,
    /// Sample spacing along a candidate link (drop-cutter probe density).
    pub sampling: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
}

/// The one FINISHING configuration of [`relink_fragments`], built once and
/// shared by every finishing generator (G-LINKSTAGE).
///
/// Before this existed each call site spelled the same nine fields out by
/// hand, and they disagreed: the unified finish took its ceiling from
/// `ctx.initial_stock` while the scallop passed `link_ceiling: None`, so on a
/// `FromRemainingStock` island pass the scallop could never take a lifted
/// finger-to-finger hop and every one of those junctions paid a full safe-Z
/// retract round trip. The ceiling is also the SAFETY half: without it a
/// surface-riding link rides the MESH, which on a rest-driven pass sits below
/// the standing material — a lateral feed at cut depth straight through stock,
/// the G-ISOCLIPRAPID shape.
///
/// The fields here are the ones an execution context knows and a generator
/// does not; [`LinkGeometry`] carries the ones the generator knows.
#[derive(Debug, Clone, Copy)]
pub struct FinishingLinkStage<'a> {
    /// Candidate CAP on the XY gap a link may span. `0.0` disables the stage
    /// — the byte-identity dial every family that has not opted in keeps.
    pub hookup_distance: f64,
    /// The op's INPUT stock, when a simulated snapshot is in scope. `None` on
    /// a fresh-stock pass, where the mesh IS the material — that arm is
    /// byte-identical to the legacy surface-riding link.
    pub link_ceiling: Option<LinkCeiling<'a>>,
    /// The op's machining regions. A surface-riding link is a cutting feed,
    /// so it may not leave them; a LIFTED one may (see
    /// [`RelinkParams::airborne_links_may_leave_territory`]).
    pub boundary: Option<&'a crate::geometry::region_set::RegionSet<'a>>,
    /// Machine envelope used to cost a candidate against the retract it would
    /// replace (F-034 integrator).
    pub link_kinematics: Option<&'a crate::machine::kinematics::LinkKinematics>,
}

impl<'a> FinishingLinkStage<'a> {
    /// Build the relink parameters for one generator's pass.
    ///
    /// The three op PRIORS are fixed here, on purpose — they are what makes
    /// this the finishing configuration rather than the engraving one:
    ///
    /// * `reorder: true` — a link is only possible when the next fragment is
    ///   CLOSE, so ordering and linking are the same lever applied twice. It
    ///   is forward-only: a fragment is never reversed, so cut direction, and
    ///   with it climb/conventional, is preserved.
    /// * `flush_ride: true` — flush ground under a finishing pass is the
    ///   PRIOR pass's machined output, so riding it is a sub-cusp skim, not a
    ///   slide across the raw workpiece face.
    /// * `airborne_links_may_leave_territory: true` — a finishing boundary is
    ///   a region polygon whose job is to confine CUTTING. On a dendritic
    ///   island the straight line between two fragments of the same region
    ///   leaves it constantly.
    #[must_use]
    pub fn params(&self, geom: &LinkGeometry) -> RelinkParams<'a> {
        RelinkParams {
            hookup_distance: self.hookup_distance,
            stock_to_leave: geom.stock_to_leave,
            sampling: geom.sampling,
            feed_rate: geom.feed_rate,
            plunge_rate: geom.plunge_rate,
            safe_z: geom.safe_z,
            link_kinematics: self.link_kinematics,
            reorder: true,
            boundary: self.boundary,
            link_ceiling: self.link_ceiling,
            flush_ride: true,
            airborne_links_may_leave_territory: true,
        }
    }
}

/// How close two targets must be, in every axis, for a fragment to count as
/// a CLOSED loop. The generators emit the closing move onto the stored start
/// point itself, so this is an exact-equality test with room for one f64
/// round trip, never a tolerance the caller can tune.
const LOOP_CLOSE_EPS_MM: f64 = 1e-6;

/// Upper bound on the extra candidate points a closed loop contributes to the
/// reorder picker. Enough to measure a ring by its nearest arc rather than by
/// its arbitrary start vertex; small enough that a 600-ring pass adds tens of
/// thousands of points, not millions.
const LOOP_SEED_POINTS: usize = 16;

/// Evenly spaced XY samples of a closed loop, for the reorder picker.
///
/// Deterministic by construction (a fixed stride over the stored points), so
/// two runs of the same generator seed the picker identically.
fn loop_seed_points(moves: &[(usize, crate::toolpath::Move)]) -> Vec<(f64, f64)> {
    let n = moves.len();
    if n <= 2 {
        return Vec::new();
    }
    let stride = n.div_ceil(LOOP_SEED_POINTS).max(1);
    moves
        .iter()
        .step_by(stride)
        .map(|(_, m)| (m.target.x, m.target.y))
        .collect()
}

/// Rotate a CLOSED-LOOP fragment so it begins at the loop point nearest
/// `from`, and closes back there.
///
/// Returns `None` — leave the fragment exactly as it is — whenever the
/// rotation cannot be proved sound or would change nothing:
///
/// * fewer than four moves (nothing to rotate onto);
/// * any move that is not `Linear`. An arc carries I/J offsets measured from
///   its own start, which a rotation would have to recompute. The stage runs
///   BEFORE the dressups, so no shipped caller can reach this — it is a guard
///   on a generic kernel, not a case;
/// * the last target is not the first one, i.e. the caller called this an
///   open run's kind by mistake;
/// * the nearest point is already the start.
///
/// # What the rotation preserves, and what it assumes
///
/// The SET of cut positions is preserved exactly: the loop's points are
/// re-ordered, never resampled, and the closing move is re-pointed at the new
/// start. Cut DIRECTION is preserved: the walk goes forward around the loop
/// from the new start, so climb/conventional does not flip.
///
/// It ASSUMES the loop's body moves share one feed rate and one intent, and
/// re-stamps every rotated move with the attributes of `moves[1]` — the input
/// fragment's first body cut. It has to: `moves[0]` is the ENTRY (plunge rate,
/// `EntryPlunge`), and after a rotation that move sits in the middle of the
/// cut, where a plunge feed would be wrong. Every generator that declares
/// [`FragmentKind::ClosedLoop`] emits one uniform ring, and the position-0
/// attributes are unused anyway — [`relink_fragments`] re-emits that move
/// itself, from its target alone, as either a link feed or a plunge.
fn rotate_closed_loop(
    moves: &[(usize, crate::toolpath::Move)],
    from: P3,
) -> Option<Vec<(usize, crate::toolpath::Move)>> {
    use crate::toolpath::{Move, MoveType};
    if moves.len() < 4 {
        return None;
    }
    if !moves
        .iter()
        .all(|(_, m)| matches!(m.move_type, MoveType::Linear { .. }))
    {
        return None;
    }
    let start = moves.first()?.1.target;
    let close = moves.last()?.1.target;
    if (close.x - start.x).abs() > LOOP_CLOSE_EPS_MM
        || (close.y - start.y).abs() > LOOP_CLOSE_EPS_MM
        || (close.z - start.z).abs() > LOOP_CLOSE_EPS_MM
    {
        return None;
    }
    // The loop's distinct points; the final move closes back onto index 0.
    let cycle = moves.get(..moves.len() - 1)?;
    // Only the two Copy attributes are needed; the `Move` itself is not
    // cloned.
    let (body_type, body_intent) = {
        let body = moves.get(1)?;
        (body.1.move_type, body.1.intent)
    };
    let mut best = 0usize;
    let mut best_d = f64::INFINITY;
    for (k, (_, m)) in cycle.iter().enumerate() {
        let d = (m.target.x - from.x).powi(2) + (m.target.y - from.y).powi(2);
        if d < best_d {
            best_d = d;
            best = k;
        }
    }
    if best == 0 {
        return None;
    }
    let n = cycle.len();
    let mut out: Vec<(usize, Move)> = Vec::with_capacity(moves.len());
    for step in 0..n {
        let (old, mv) = cycle.get((best + step) % n)?;
        out.push((
            *old,
            Move {
                target: mv.target,
                move_type: body_type,
                intent: body_intent,
            },
        ));
    }
    // Close onto the new start, carrying the input's own closing index so the
    // provenance stays a bijection over this fragment's moves.
    let (close_old, _) = moves.last()?;
    let new_start = cycle.get(best)?.1.target;
    out.push((
        *close_old,
        Move {
            target: new_start,
            move_type: body_type,
            intent: body_intent,
        },
    ));
    Some(out)
}

/// Inputs for [`relink_fragments`].
#[derive(Debug, Clone, Copy)]
pub struct RelinkParams<'a> {
    /// Candidate CAP on the XY gap a surface link may span. `0.0` disables
    /// linking entirely (the pass then only reorders, if asked).
    pub hookup_distance: f64,
    pub stock_to_leave: f64,
    /// Sample spacing along a candidate link (drop-cutter probe density).
    pub sampling: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    /// When `Some`, a candidate link is additionally costed against the
    /// retract it would replace with the F-034 integrator and only kept
    /// when it is actually faster — the same decision
    /// [`crate::pencil::emit_paths`] makes. `None` keeps any gouge-safe
    /// link within `hookup_distance`.
    pub link_kinematics: Option<&'a crate::machine::kinematics::LinkKinematics>,
    /// Visit fragments nearest-first (from the previous fragment's exit)
    /// instead of in emitted order. Forward-only: a fragment is never
    /// reversed, so its cut direction — and therefore climb/conventional —
    /// is preserved.
    pub reorder: bool,
    /// Territory the link may cross. Every sampled link point must lie
    /// inside it or the candidate is refused.
    ///
    /// NOT optional in spirit: a surface link is a CUTTING feed, so one
    /// that leaves the region it belongs to machines ground the op was
    /// confined away from — the selective-finishing gouge class the
    /// boundary exists to prevent. `unified_finish::choose_link` has
    /// enforced exactly this for region-to-region links since P2.d; the
    /// first version of THIS pass omitted it and the wanaka ×2 COLUMNS
    /// gate caught it (shallow `<-.5` over-cut columns 1 218 → 4 068,
    /// worst column −3.0 → −3.9 mm). On dendritic rest islands a straight
    /// line between two fragments of the SAME region leaves that region
    /// constantly, so intra-region linking needs the check just as much as
    /// inter-region linking does.
    ///
    /// `None` disables the check — only correct when the caller knows the
    /// fragments span no excluded territory.
    pub boundary: Option<&'a crate::geometry::region_set::RegionSet<'a>>,
    /// Lift every link sample clear of standing material, and refuse the
    /// link outright when that clearance reaches `safe_z`. See
    /// [`LinkCeiling`]. `None` keeps the legacy surface-riding link — the
    /// byte-identical behaviour the finishing families rely on, where the
    /// mesh already IS the material.
    pub link_ceiling: Option<LinkCeiling<'a>>,
    /// In ceiling mode, permit a junction whose whole hop reads FLUSH
    /// (dexel material at the mesh surface) to ride the surface instead of
    /// taking the lifted shape. This is an OP PRIOR, not a geometry fact —
    /// the dexel cannot distinguish "machined down to the surface" from
    /// "raw stock that happens to sit at surface height":
    ///
    /// - A FromRemainingStock FINISHING pass sets `true`: flush ground is
    ///   the prior pass's machined output, and riding it is a harmless
    ///   sub-cusp skim (the staple hedge this removes: 2,569 lifted links
    ///   on one wanaka raster, two clearance legs each for sub-mm hops).
    /// - An ENGRAVING pass (project_curve, the original ceiling caller)
    ///   sets `false`: its flush ground is the RAW WORKPIECE FACE, and a
    ///   fed slide across it drags the cutter over stock the op must not
    ///   touch — the exact contract `stock_safety_links_clear_standing_material`
    ///   pins (ceiling + clearance everywhere, flats included).
    ///
    /// Ignored when `link_ceiling` is `None`.
    pub flush_ride: bool,
    /// Let a LIFTED (ceiling-mode) link cross ground [`Self::boundary`]
    /// excludes. Like [`Self::flush_ride`] this is an OP PRIOR, and for the
    /// same reason it cannot be inferred from the geometry: being airborne
    /// answers the GOUGE question ("does this link cut anything?"), and the
    /// boundary asks a second, independent TERRITORY question ("may the tool
    /// be over there at all?"). A machining boundary also encodes keep-outs
    /// and fixtures, and [`LinkCeiling`] reads its clearance from the dexel
    /// stock, which need not model a clamp at all — so "it clears the
    /// material the dexel knows about" is not "it clears the workholding".
    /// Only the op knows which of the two its boundary means.
    ///
    /// - A FINISHING pass over a decomposed region sets `true`: its boundary
    ///   is the region polygon, whose only job is to keep CUTTING confined.
    ///   On a dendritic rest island the straight line between two fragments
    ///   of the same region leaves that region constantly, and vetoing those
    ///   airborne hops turned nearly every finger-to-finger junction into a
    ///   full safe-Z retract (measured on the confined wanaka tier 1: 17,083
    ///   intra-node retract trips, 363.8 m of rapids against 86.8 m of
    ///   cutting).
    /// - An ENGRAVING pass (project_curve, the original ceiling caller) sets
    ///   `false`: its boundary is the operation's machining territory, which
    ///   the operator may have drawn around a clamp or a keep-out, so an
    ///   airborne traverse across it is still a refusal
    ///   (`project_curve_chaining::a_link_may_not_leave_the_machining_boundary`
    ///   pins it). This is the conservative value; a caller with no opinion
    ///   sets `false`.
    ///
    /// The exemption is conjunctive with the link's actual shape: a
    /// SURFACE-RIDING link (`link_ceiling: None`, or a ceiling link that took
    /// the [`Self::flush_ride`] arm) is a CUTTING feed and stays vetoed
    /// everywhere, whatever this flag says.
    pub airborne_links_may_leave_territory: bool,
}

/// What [`relink_fragments`] did.
#[derive(Debug, Clone, Default)]
pub struct RelinkReport {
    pub fragments: usize,
    /// Junctions joined by a link of either kind (no retract). Always
    /// [`Self::at_depth_links`] + [`Self::clearance_hops`]; kept as the sum so
    /// every existing reader keeps its meaning.
    pub surface_links: usize,
    /// TIER (a) — junctions joined by a link that arrives AT CUTTING DEPTH.
    ///
    /// The acceptance measure. Only this tier removes an ENTRY: the tool
    /// never leaves the material, so the next fragment needs no plunge, no
    /// ramp and no helix. On the wanaka pencil baseline an entry costs 7.2 s
    /// against 0.16 s of cutting per fragment, so a link that removes a
    /// retract but still lands from above scores zero there — which is why
    /// this is counted apart from [`Self::clearance_hops`], not with it.
    pub at_depth_links: usize,
    /// TIER (b) — junctions joined by a link that LIFTED to the local stock
    /// ceiling and descended again.
    ///
    /// Cheaper than a safe-Z retract (it clears only what stands between the
    /// two fragments) but it still arrives from above, so the fragment still
    /// pays a descent. Counted separately for exactly that reason: on a
    /// retract-bound pass (the island scallop and raster) a hop is a real
    /// win; on an entry-bound pass (pencil) it is not.
    pub clearance_hops: usize,
    /// TIER (c) — junctions that fell back to retract → traverse → plunge.
    pub retract_links: usize,
    /// Closed-loop fragments the stage ROTATED to start near the previous
    /// fragment's exit. `0` for every caller that declares no
    /// [`FragmentKind`], which is the byte-identical legacy arm.
    pub rotated_loops: usize,
    /// Junctions rejected because the gap exceeded `hookup_distance`.
    pub too_far: usize,
    /// Junctions where the tool would have lost surface contact.
    pub off_surface: usize,
    /// Junctions where the link was gouge-safe but slower than retracting.
    pub slower_than_retract: usize,
    /// Junctions where the link would have left `RelinkParams::boundary`.
    pub outside_boundary: usize,
    /// Junctions refused because [`RelinkParams::link_ceiling`] put the
    /// clearance height at or above `safe_z`: there is nothing left for a
    /// fed link to save, so the retract is kept. Structurally always `0`
    /// for a caller that passes `link_ceiling: None`.
    pub ceiling_above_safe_z: usize,
}

/// Rewrite a toolpath's inter-fragment junctions, replacing
/// retract → traverse → plunge with a gouge-checked surface-following link
/// wherever one is safe and (optionally) faster.
///
/// WHY THIS EXISTS, measured: on wanaka ×2 the unified rest-clearer spends
/// 17 077 s in rapids across 12 780 fragments — roughly 1.3 s per junction,
/// and almost all of it is the two ~30 mm Z legs, not the XY hop between
/// them. Reordering shortens the hop (measured 8.6 mm → 2.8 mm) but cannot
/// remove a single Z leg. Only keeping the tool down can. See
/// `planning/unified_v3_design.md` §9.
///
/// A "fragment" is a maximal run of non-`Rapid` moves — the same split
/// [`crate::dressup::tsp::optimize_rapid_order`] uses, so the two passes agree on
/// what is atomic. Fragment interiors are copied VERBATIM; only the
/// junctions between them are rewritten, which is what makes the cut
/// geometry provably unchanged except at the (previously airborne)
/// junctions themselves.
///
/// Linking is refused whenever the previous fragment ends at or above
/// `safe_z` — that fragment retracted with a FEED rather than a rapid, so
/// its exit is not a point on the surface and a link from it would descend
/// diagonally through material.
///
/// With [`RelinkParams::link_ceiling`] set, the link does not ride the
/// surface at all: it leaves the cut vertically, traverses at
/// `max(surface, standing material)` plus
/// [`crate::toolpath::PLUNGE_CLEARANCE_MM`],
/// and re-enters vertically — and is refused entirely once that clearance
/// reaches `safe_z`. That is what makes the pass usable on an op whose mesh
/// is NOT the material (engraving on raw stock); see [`LinkCeiling`].
///
/// # Provenance (C1)
///
/// This transform moves indices — it deletes the input's junction rapids,
/// emits link feeds in their place, and (under
/// [`RelinkParams::reorder`]) permutes whole fragments. It therefore hands
/// back a [`Transformed`] carrying a [`MoveProvenance`] rather than the
/// hand-rolled `old -> new` vector it used to expose, so every
/// index-carrying channel the call site owns is brought along by the type
/// system instead of by convention. `reorder` picks the provenance FLAVOUR:
/// a permutation carries the foreign-intrusion drop rule a plain remap
/// cannot express — and so does a loop ROTATION, which is why
/// [`relink_fragments_with_kinds`] reports `Permutation` whenever it rotates,
/// even with `reorder: false`.
pub fn relink_fragments(
    annotated: AnnotatedToolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RelinkParams<'_>,
) -> (Transformed, RelinkReport) {
    relink_fragments_with_kinds(annotated, mesh, index, cutter, params, None)
}

/// [`relink_fragments`] told what each fragment IS, so it may rotate a closed
/// loop to start near the previous fragment's exit.
///
/// `fragment_kinds` is one [`FragmentKind`] per fragment, in EMITTED order —
/// the order [`relink_fragments`] itself splits them out of the move list, so
/// a caller builds it in the same loop that emits the runs. A slice whose
/// length disagrees with the fragment count is REFUSED (logged, then treated
/// as all-[`FragmentKind::OpenRun`]) rather than applied at an offset: a
/// mis-aligned kind would rotate the wrong fragment, and a silent
/// off-by-one in a cutting transform is not an acceptable failure mode.
///
/// `None` is byte-identical to [`relink_fragments`] — no rotation is
/// attempted, and the visiting order is the one the plain entry point
/// produces.
///
/// The kinds ride a PARAMETER rather than a [`RelinkParams`] field on
/// purpose: `RelinkParams` is spelled out as an exhaustive struct literal at
/// 27 sites across the crate and its tests, and a new field would rewrite
/// every one of them for a value that only two callers can supply.
#[allow(clippy::too_many_arguments)]
pub fn relink_fragments_with_kinds(
    annotated: AnnotatedToolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RelinkParams<'_>,
    fragment_kinds: Option<&[FragmentKind]>,
) -> (Transformed, RelinkReport) {
    use crate::toolpath::{MoveIntent, MoveType, Toolpath};

    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;
    let n_in = toolpath.moves.len();

    // ── split into fragments, remembering each move's original index ────
    struct Fragment {
        moves: Vec<(usize, crate::toolpath::Move)>,
    }
    /// Which rung of the link ladder a kept candidate took. The distinction
    /// the acceptance measure needs — see [`RelinkReport::at_depth_links`].
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LinkTier {
        /// (a) Arrives at cutting depth. Removes the next fragment's entry.
        AtDepth,
        /// (b) Lifts to the local stock ceiling and descends again. Cheaper
        /// than a safe-Z retract; the entry survives.
        ClearanceHop,
    }
    let mut frags: Vec<Fragment> = Vec::new();
    let mut cur: Vec<(usize, crate::toolpath::Move)> = Vec::new();
    for (i, mv) in toolpath.moves.iter().enumerate() {
        if matches!(mv.move_type, MoveType::Rapid) {
            if !cur.is_empty() {
                frags.push(Fragment {
                    moves: std::mem::take(&mut cur),
                });
            }
        } else {
            cur.push((i, mv.clone()));
        }
    }
    if !cur.is_empty() {
        frags.push(Fragment { moves: cur });
    }

    let mut report = RelinkReport {
        fragments: frags.len(),
        ..RelinkReport::default()
    };
    if frags.len() < 2 {
        return (
            Transformed::index_preserving(AnnotatedToolpath {
                toolpath,
                spans,
                spans_valid,
                planner_engagement,
                rest_grid,
                rest_regions,
            }),
            report,
        );
    }

    // Input indices that are NOT part of any fragment — the junction
    // rapids. Each belongs to the fragment it precedes, so it maps onto the
    // junction moves that replaced it; the ones after the last fragment map
    // onto the closing retract.
    // Stored INVERTED — fragment -> the input rapid indices it owns — rather
    // than as a per-move `Option<usize>`. The emit loop below needs "which
    // rapids belong to fragment `fi`", and answering that from the forward
    // map costs a full `n_in` scan per fragment: at wanaka's 12,780 fragments
    // against ~10^5 moves that is ~10^9 iterations to write information this
    // pass already knows (PERF_REVIEW G6). Both directions are built by the
    // same single O(n_in) walk; only the storage shape changed.
    let mut rapids_of_frag: Vec<Vec<usize>> = vec![Vec::new(); frags.len()];
    {
        let mut next_frag = 0usize;
        for (i, mv) in toolpath.moves.iter().enumerate() {
            if matches!(mv.move_type, MoveType::Rapid) {
                // SAFETY: `next_frag` is only advanced past a fragment whose
                // last index has been seen, so it stays in `0..=frags.len()`.
                if let Some(slot) = rapids_of_frag.get_mut(next_frag) {
                    slot.push(i);
                }
            } else if frags
                .get(next_frag)
                .and_then(|f| f.moves.last())
                .is_some_and(|(last, _)| *last == i)
            {
                next_frag += 1;
            }
        }
    }

    let entry_of = |f: &Fragment| -> P3 { f.moves.first().map_or(P3::origin(), |(_, m)| m.target) };
    let first_of = |ms: &[(usize, crate::toolpath::Move)]| -> P3 {
        ms.first().map_or(P3::origin(), |(_, m)| m.target)
    };
    let last_of = |ms: &[(usize, crate::toolpath::Move)]| -> P3 {
        ms.last().map_or(P3::origin(), |(_, m)| m.target)
    };
    let xy_gap = |a: P3, b: P3| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();

    // ── fragment kinds ──────────────────────────────────────────────────
    // One kind per fragment, in the order this pass split them out. A slice
    // of the wrong length is refused rather than applied at an offset — see
    // `relink_fragments_with_kinds`.
    let kinds: Option<&[FragmentKind]> = match fragment_kinds {
        Some(k) if k.len() == frags.len() => Some(k),
        Some(k) => {
            tracing::warn!(
                declared = k.len(),
                fragments = frags.len(),
                "relink: fragment-kind slice does not match the fragment count; \
                 treating every fragment as an open run"
            );
            None
        }
        None => None,
    };
    let kind_of = |fi: usize| -> FragmentKind {
        kinds
            .and_then(|k| k.get(fi).copied())
            .unwrap_or(FragmentKind::OpenRun)
    };

    // ── visiting order ──────────────────────────────────────────────────
    // Was a Θ(fragments²) scan: at the 12,780 fragments this op reaches on
    // the wanaka workload that is ~163 M `xy_gap` evaluations (PERF_REVIEW
    // G5). `NearestPicker` returns the lexicographic minimum of
    // `(xy_gap, fragment index)`, which is exactly what the scan's
    // "first strict improvement in index order" computed, so the visiting
    // order — and therefore the emitted toolpath — is unchanged. The
    // `usize::MAX` sentinel and its `break` are kept verbatim: they are what
    // a non-finite fragment entry used to do, and that must stay true.
    //
    // Ordering and rotation are ONE interleaved walk, not two passes. The
    // picker's next query is seeded from the previous fragment's EXIT, and a
    // rotated loop exits somewhere else than the one it was emitted with — so
    // computing the whole order first and rotating afterwards would steer
    // every later pick from a position the tool never reaches. With
    // `kinds: None` nothing rotates, so this walk produces exactly the order
    // the two-pass form did.
    let n_frags = frags.len();
    let mut picker = if params.reorder {
        let mut picker = crate::geometry::nn_order::NearestPicker::new(
            crate::geometry::nn_order::Metric::Euclid,
            n_frags,
        );
        for (j, f) in frags.iter().enumerate() {
            let e = entry_of(f);
            picker.push(j, e.x, e.y);
            // A CLOSED LOOP may be entered anywhere on its circumference, so
            // measuring it by its arbitrary start vertex alone under-rates a
            // ring whose nearest point is half a diameter away from it. The
            // picker takes several points per owner and reports the minimum
            // over them (`nn_order`'s own contract), so seed the loop with a
            // decimated, deterministic sample of itself.
            if matches!(kind_of(j), FragmentKind::ClosedLoop) {
                for (x, y) in loop_seed_points(&f.moves) {
                    picker.push(j, x, y);
                }
            }
        }
        picker.build();
        picker.remove(0);
        Some(picker)
    } else {
        None
    };

    // ── re-emit ─────────────────────────────────────────────────────────
    let mut out = Toolpath::new();
    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> = vec![None; n_in];
    let mut prev_exit: Option<P3> = None;
    let mut visited = 0usize;
    let mut next_fi: Option<usize> = (n_frags > 0).then_some(0);
    while let Some(fi) = next_fi {
        let Some(frag) = frags.get(fi) else { break };
        // ROTATION (tier-independent): a closed loop starts wherever the
        // generator's offset library happened to start it. Once the tool has
        // a position, the loop may begin at its nearest point instead, which
        // is what turns a `too_far` ring junction into a candidate at all.
        let rotated = match (prev_exit, kind_of(fi)) {
            (Some(from), FragmentKind::ClosedLoop) => rotate_closed_loop(&frag.moves, from),
            _ => None,
        };
        if rotated.is_some() {
            report.rotated_loops += 1;
        }
        let frag_moves: &[(usize, crate::toolpath::Move)] =
            rotated.as_deref().unwrap_or(frag.moves.as_slice());
        let entry = first_of(frag_moves);
        let junction_start = out.moves.len();

        let link = prev_exit.and_then(|from| {
            if params.hookup_distance <= 0.0 {
                return None;
            }
            // The previous fragment retracted with a feed, so its exit is
            // not a surface point — refuse (see the doc comment).
            if from.z >= params.safe_z - 1e-6 {
                report.off_surface += 1;
                return None;
            }
            let gap = xy_gap(from, entry);
            if gap <= 1e-6 {
                // The two fragments touch: the tool is already standing on
                // the next entry, so nothing travels and nothing descends.
                return Some((Vec::new(), LinkTier::AtDepth));
            }
            if gap > params.hookup_distance {
                report.too_far += 1;
                return None;
            }
            let Some(pts) = build_surface_link(
                from,
                entry,
                mesh,
                index,
                cutter,
                params.stock_to_leave,
                params.sampling,
            ) else {
                report.off_surface += 1;
                return None;
            };
            // Mirrors `unified_finish::choose_link`: a SURFACE-RIDING link is
            // a CUTTING feed, so it must not leave the territory this op is
            // confined to. Endpoints are cut positions and trivially
            // inside; the interior samples carry the test.
            //
            // A CEILING link MAY be exempt (G-LINKVETO, operator-observed
            // 2026-08-27): it does not ride the surface — it exits
            // vertically, traverses at `max(surface, standing stock) +
            // clearance` sampled along the whole hop, and re-enters
            // vertically, so it cuts nothing whatever territory it crosses.
            // On a dendritic island the straight line between two fragments
            // of the SAME region leaves that region constantly, and this
            // veto was turning nearly every short finger-to-finger hop into
            // a full safe-Z retract round trip (measured on the confined
            // wanaka tier 1: 17,083 intra-node retract trips, 363.8 m of
            // rapids against 86.8 m of cutting).
            //
            // BOTH halves are required, and the second one cannot be read
            // off the geometry: airborne answers the GOUGE question, not the
            // TERRITORY one, so the op has to declare whether its boundary
            // is a cut-confinement polygon (exempt) or a keep-out/fixture
            // envelope (never exempt). See
            // `RelinkParams::airborne_links_may_leave_territory`.
            let airborne_exempt =
                params.link_ceiling.is_some() && params.airborne_links_may_leave_territory;
            if !airborne_exempt
                && let Some(boundary) = params.boundary
                && !pts
                    .iter()
                    .all(|p| boundary.contains(&crate::geo::P2::new(p.x, p.y)))
            {
                report.outside_boundary += 1;
                return None;
            }
            // Stock-aware ceiling. Applied AFTER the boundary check (which
            // is an XY test, so lifting cannot change its answer) and
            // BEFORE the kinematics costing, so the cost model prices the
            // geometry that will actually be emitted.
            // TIER, decided here: without a ceiling the link rides the mesh
            // at cut depth (a); with one it is (a) only when the whole hop
            // reads flush, and otherwise the lifted clearance hop (b).
            let (pts, tier) = match params.link_ceiling {
                None => (pts, LinkTier::AtDepth),
                Some(ceiling) => {
                    let surface_z_at = |x: f64, y: f64| {
                        let cl = point_drop_cutter(x, y, mesh, index, cutter);
                        if cl.contacted {
                            cl.z
                        } else {
                            f64::NEG_INFINITY
                        }
                    };
                    let samples: Vec<(f64, f64, f64)> =
                        std::iter::once((from.x, from.y, surface_z_at(from.x, from.y)))
                            .chain(pts.iter().map(|p| (p.x, p.y, p.z - params.stock_to_leave)))
                            .chain(std::iter::once((
                                entry.x,
                                entry.y,
                                surface_z_at(entry.x, entry.y),
                            )))
                            .collect();
                    // Pencil's prior, applied PER JUNCTION (operator-observed
                    // 2026-08-27): where the dexel says the stock is already
                    // AT the surface along the whole hop, the mesh IS the
                    // material — the legacy surface-riding link is correct
                    // and strictly cheaper than a lift (on the wanaka tier-1
                    // raster, 2,569 links each paid two clearance legs for a
                    // sub-millimetre row hop: a hedge of staples along every
                    // raster edge). A surface-riding link is a CUTTING feed,
                    // so the territory veto — which
                    // `airborne_links_may_leave_territory` can waive only for
                    // LIFTED links — applies here after all, and is re-run
                    // below because the exemption above may have skipped it.
                    //
                    // `material_top` is a conservative MAX over the tool
                    // disc, so on sloped ground it reads above the centre
                    // surface even with zero standing stock — there the test
                    // fails and the safe lifted shape stays. Deliberate: the
                    // flush ride is a flat-ground optimisation, never a
                    // slope gamble.
                    //
                    // This one stays the FLAT-CYLINDER read on purpose. The
                    // profile-aware ceiling (`required_tip_z`, used by
                    // `clear_z` below) reads LOWER wherever the tool rises
                    // above its tip, and a lower reading here would widen the
                    // flush arm — i.e. ride the surface more often. Relaxing
                    // the LIFT HEIGHT is provably safe; relaxing the decision
                    // to lift at all is not the same question.
                    const FLUSH_EPS_MM: f64 = 0.15;
                    let flush = params.flush_ride
                        && samples.iter().all(|&(x, y, sz)| {
                            sz.is_finite() && ceiling.material_top(x, y) <= sz + FLUSH_EPS_MM
                        });
                    if flush {
                        if let Some(boundary) = params.boundary
                            && !pts
                                .iter()
                                .all(|p| boundary.contains(&crate::geo::P2::new(p.x, p.y)))
                        {
                            report.outside_boundary += 1;
                            return None;
                        }
                        (pts, LinkTier::AtDepth)
                    } else {
                        // Exit lift, then the interior samples, then the
                        // re-entry lift: the traverse is entirely at
                        // clearance and both ends of it are vertical, so
                        // nothing standing between the two fragments is
                        // crossed at cut depth. The degenerate case — a gap
                        // shorter than one sample spacing, where
                        // `build_surface_link` yields no interior points at
                        // all — is the same rule with an empty middle, not a
                        // special case that feeds across at depth.
                        let mut lifted: Vec<P3> = Vec::with_capacity(samples.len());
                        for &(x, y, surface_z) in &samples {
                            let z = ceiling.clear_z(cutter, x, y, surface_z);
                            if z >= params.safe_z - 1e-6 {
                                // Clearing the standing material costs the
                                // whole retract anyway — keep the rapid,
                                // which is faster than feeding to the same
                                // height.
                                report.ceiling_above_safe_z += 1;
                                return None;
                            }
                            lifted.push(P3::new(x, y, z));
                        }
                        (lifted, LinkTier::ClearanceHop)
                    }
                }
            };
            match params.link_kinematics {
                Some(lk) => {
                    let mut costed = pts.clone();
                    costed.push(entry);
                    let surface_t = crate::machine::kinematics::surface_link_time(
                        from,
                        &costed,
                        params.feed_rate,
                        &lk.kinematics,
                        lk.max_feed_mm_min,
                        lk.rapid_feed_mm_min,
                    );
                    let retract_t = crate::machine::kinematics::retract_link_time(
                        from,
                        entry,
                        params.safe_z,
                        None,
                        params.plunge_rate,
                        &lk.kinematics,
                        lk.max_feed_mm_min,
                        lk.rapid_feed_mm_min,
                    );
                    if surface_t <= retract_t {
                        Some((pts, tier))
                    } else {
                        report.slower_than_retract += 1;
                        None
                    }
                }
                None => Some((pts, tier)),
            }
        });

        match (link, prev_exit) {
            (Some((pts, tier)), Some(_)) => {
                report.surface_links += 1;
                match tier {
                    LinkTier::AtDepth => report.at_depth_links += 1,
                    LinkTier::ClearanceHop => report.clearance_hops += 1,
                }
                for p in &pts {
                    out.feed_to_with_intent(*p, params.feed_rate, MoveIntent::Linking);
                }
                // The fragment's own first move lands on `entry`; re-emit it
                // as a link so the run reads as one stay-down chain.
                out.feed_to_with_intent(entry, params.feed_rate, MoveIntent::Linking);
            }
            (_, Some(from)) => {
                report.retract_links += 1;
                out.rapid_to_with_intent(
                    P3::new(from.x, from.y, params.safe_z),
                    MoveIntent::Retract,
                );
                out.rapid_to_with_intent(
                    P3::new(entry.x, entry.y, params.safe_z),
                    MoveIntent::Linking,
                );
                out.feed_to_with_intent(entry, params.plunge_rate, MoveIntent::EntryPlunge);
            }
            (_, None) => {
                // First fragment: approach exactly as the input did.
                out.rapid_to_with_intent(
                    P3::new(entry.x, entry.y, params.safe_z),
                    MoveIntent::Linking,
                );
                out.feed_to_with_intent(entry, params.plunge_rate, MoveIntent::EntryPlunge);
            }
        }

        let junction_end = out.moves.len();
        // Every junction rapid this fragment used to be reached through was
        // REPLACED by the moves emitted just above — that is where it went,
        // and saying so is the whole point of the provenance.
        for &old in rapids_of_frag.get(fi).map_or(&[][..], Vec::as_slice) {
            if let Some(slot) = old_to_new.get_mut(old) {
                *slot = Some(junction_start..junction_end);
            }
        }

        // The last move re-emitted above lands on `entry`, so it stands in
        // for `frag_moves[0]` — the plunge (or first cut) that used to do so.
        // Under a rotation that is the move carrying the loop's NEW start
        // point, not the input's own first move; both are moves of this
        // fragment, and `rotate_closed_loop` keeps the input indices a
        // bijection, so every input move is still accounted for exactly once.
        if let Some((old, _)) = frag_moves.first()
            && let Some(slot) = old_to_new.get_mut(*old)
        {
            let last = junction_end.saturating_sub(1);
            *slot = Some(last..junction_end);
        }
        for (old, mv) in frag_moves.iter().skip(1) {
            let new = out.moves.len();
            out.moves.push(mv.clone());
            if let Some(slot) = old_to_new.get_mut(*old) {
                *slot = Some(new..new + 1);
            }
        }
        let exit = last_of(frag_moves);
        prev_exit = Some(exit);
        visited += 1;

        // Pick the next fragment from where the tool ACTUALLY stopped, which
        // under a rotation is not where the input fragment ended.
        next_fi = if visited >= n_frags {
            None
        } else {
            match picker.as_mut() {
                Some(picker) => match picker.nearest(exit.x, exit.y) {
                    // The `usize::MAX` sentinel of the two-pass form: a
                    // non-finite fragment entry ended the walk, and that must
                    // stay true.
                    Some((j, d)) if d < f64::INFINITY && j < n_frags => {
                        picker.remove(j);
                        Some(j)
                    }
                    _ => None,
                },
                None => Some(fi + 1),
            }
        };
    }

    if let Some(end) = prev_exit {
        let closing = out.moves.len();
        out.rapid_to_with_intent(P3::new(end.x, end.y, params.safe_z), MoveIntent::Retract);
        // Trailing rapids (the input's own closing retract) land on it.
        for (old, slot) in old_to_new.iter_mut().enumerate() {
            if slot.is_none()
                && matches!(
                    toolpath.moves.get(old).map(|m| &m.move_type),
                    Some(MoveType::Rapid)
                )
            {
                *slot = Some(closing..closing + 1);
            }
        }
    }

    let new_n_moves = out.moves.len();
    let remap = MoveRemap { old_to_new };
    let spans = if spans_valid {
        remap.remap_spans(&spans, new_n_moves)
    } else {
        spans
    };
    let annotated = AnnotatedToolpath {
        toolpath: out,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    };
    // A reorder needs the drop rule a plain remap cannot state — see
    // `MoveProvenance::Permutation`.
    //
    // A ROTATION needs it for the same reason, and it is not covered by
    // `params.reorder`: rotating a closed loop scatters that fragment's own
    // move indices, so a claim over part of it maps onto a bounding range
    // holding moves from outside the claim. `Remap` would silently widen the
    // claim to cover those strangers; `Permutation` DROPS it, which is the
    // honest answer. G-LINKSTAGE deliberately reuses this flavour rather than
    // adding a fourth: a rotation IS a permutation and carries no distinct
    // drop rule of its own.
    //
    // G-LINKTRACE correction (2026-09-10): this used to end "the scallop's
    // ring annotations anchor on the junction rapid (which no rotation
    // moves), so nothing needs re-anchoring." Both halves were wrong. The
    // junction rapid is DELETED here, and its old index is remapped onto the
    // WHOLE replacement junction, which OVERLAPS the range the next
    // fragment's first move maps onto. That is foreign intrusion at every
    // junction, produced by `reorder` alone with no rotation in play.
    // Through the RANGE query the drop rule then deleted every ring
    // annotation and this op's semantic trace collapsed. A single-move
    // anchor now asks `MoveProvenance::remap_point`, which states why the
    // rule does not reach it.
    let transformed = if params.reorder || report.rotated_loops > 0 {
        Transformed::from_permutation(annotated, remap)
    } else {
        Transformed::from_remap(annotated, remap)
    };
    (transformed, report)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::tool::BallEndmill;

    /// A symmetric V-valley running along X: two inclined planes meeting at y=0.
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

    /// Build a toolpath of `n` short cut runs along the valley floor,
    /// each separated by the retract → traverse → plunge triple
    /// `relink_fragments` is meant to remove.
    fn fragmented_valley_path(
        n: usize,
        run_len: f64,
        gap: f64,
        safe_z: f64,
    ) -> crate::toolpath::Toolpath {
        use crate::toolpath::{MoveIntent, Toolpath};
        let mut tp = Toolpath::new();
        let mut x = 1.0;
        for i in 0..n {
            if i > 0 {
                tp.rapid_to_with_intent(P3::new(x - gap, 0.0, safe_z), MoveIntent::Retract);
                tp.rapid_to_with_intent(P3::new(x, 0.0, safe_z), MoveIntent::Linking);
            } else {
                tp.rapid_to_with_intent(P3::new(x, 0.0, safe_z), MoveIntent::Linking);
            }
            tp.feed_to_with_intent(P3::new(x, 0.0, -3.0), 100.0, MoveIntent::EntryPlunge);
            let steps = 4;
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                tp.feed_to_with_intent(
                    P3::new(x + run_len * t, 0.0, -3.0),
                    500.0,
                    MoveIntent::FinishingCut,
                );
            }
            x += run_len + gap;
        }
        tp.rapid_to_with_intent(P3::new(x - gap, 0.0, safe_z), MoveIntent::Retract);
        tp
    }

    fn rapid_len(tp: &crate::toolpath::Toolpath) -> f64 {
        tp.total_rapid_distance()
    }

    /// [`relink_fragments`] for a caller holding a bare toolpath and no
    /// index-carrying channel — the contract still makes it say so.
    fn relink(
        tp: &crate::toolpath::Toolpath,
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        params: &RelinkParams<'_>,
    ) -> (crate::toolpath::Toolpath, RelinkReport) {
        use crate::trace::transform_provenance::ReconcileSet;
        let (transformed, report) = relink_fragments(
            AnnotatedToolpath::new(tp.clone()),
            mesh,
            index,
            cutter,
            params,
        );
        (
            transformed
                .reconcile(&mut ReconcileSet::empty())
                .into_inner()
                .toolpath,
            report,
        )
    }

    /// The structural claim the whole A/M7 conversion rests on: relinking
    /// rewrites JUNCTIONS, so every position the tool used to feed to is
    /// still a position the tool feeds to. Set-membership, exact, and
    /// order-independent — a reordering passes, a hole does not.
    ///
    /// Wave 11 believed this was false, on a gate that selected its
    /// baseline population by the `FinishingCut` LABEL after arc fitting
    /// had relabelled `LeadOut` geometry with it. The relinker was never the
    /// defect; see `tests/scallop_intra_pass_relink_am7.rs`.
    #[test]
    fn relink_loses_no_fed_position() {
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (out, report) = relink(&tp, &mesh, &index, &tool, &params);
        assert_eq!(report.surface_links, 5, "{report:?}");

        let fed = |t: &crate::toolpath::Toolpath| -> Vec<P3> {
            t.moves
                .iter()
                .filter(|m| !matches!(m.move_type, crate::toolpath::MoveType::Rapid))
                .map(|m| m.target)
                .collect()
        };
        let after = fed(&out);
        let missing: Vec<P3> = fed(&tp)
            .into_iter()
            .filter(|want| {
                !after.iter().any(|got| {
                    (got.x - want.x).abs() < 1e-9
                        && (got.y - want.y).abs() < 1e-9
                        && (got.z - want.z).abs() < 1e-9
                })
            })
            .collect();
        assert!(
            missing.is_empty(),
            "a relink may ADD link feeds; it may never drop a fed position: \
             {} lost, first {:?}",
            missing.len(),
            missing.first()
        );
    }

    /// The C1 half: every input move must be accounted for by the
    /// provenance, so a channel that follows it cannot be silently orphaned.
    #[test]
    fn relink_provenance_accounts_for_every_input_move() {
        use crate::trace::transform_provenance::ReconcileSet;
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);
        let n_in = tp.moves.len();

        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (transformed, _) =
            relink_fragments(AnnotatedToolpath::new(tp), &mesh, &index, &tool, &params);
        let n_out = transformed.move_count();
        let (_, prov) = transformed
            .reconcile(&mut ReconcileSet::empty())
            .into_parts();
        for i in 0..n_in {
            let r = prov.remap_range(i, i + 1, n_out);
            assert!(
                r.as_ref()
                    .is_some_and(|r| r.end <= n_out && r.start < r.end),
                "input move {i} has no place in the output: {r:?}"
            );
        }
    }

    #[test]
    fn relink_replaces_close_junctions_with_surface_links() {
        use crate::toolpath::MoveType;
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (out, report) = relink(&tp, &mesh, &index, &tool, &params);

        assert_eq!(report.fragments, 6, "fixture must present 6 fragments");
        assert_eq!(
            report.surface_links, 5,
            "every 1.5mm junction is inside the 3mm hookup and rides the \
             valley floor, so all five must link: {report:?}"
        );
        assert_eq!(report.retract_links, 0, "{report:?}");
        assert!(
            rapid_len(&out) < rapid_len(&tp),
            "removing five retract round trips must cut rapid distance: \
             {} -> {}",
            rapid_len(&tp),
            rapid_len(&out)
        );
        // Exactly one approach and one final retract survive.
        let rapids = out
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Rapid))
            .count();
        assert_eq!(
            rapids, 2,
            "one approach + one final retract is all that should remain"
        );
        // Every link point rides the surface, never safe Z.
        for m in &out.moves {
            if m.intent == crate::toolpath::MoveIntent::Linking
                && !matches!(m.move_type, MoveType::Rapid)
            {
                assert!(
                    m.target.z < safe_z - 1.0,
                    "a surface link must stay down, got z={}",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn relink_refuses_gaps_beyond_hookup_and_keeps_cut_moves() {
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(4, 4.0, 5.0, safe_z);

        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (out, report) = relink(&tp, &mesh, &index, &tool, &params);
        assert_eq!(report.surface_links, 0, "5mm > 3mm hookup: {report:?}");
        assert_eq!(report.retract_links, 3, "{report:?}");
        assert_eq!(report.too_far, 3, "{report:?}");

        // The CUT moves must be untouched — the pass only rewrites the
        // airborne junctions between fragments.
        let cuts = |t: &crate::toolpath::Toolpath| -> Vec<(String, String)> {
            t.moves
                .windows(2)
                .filter(|w| w[1].intent == crate::toolpath::MoveIntent::FinishingCut)
                .map(|w| {
                    (
                        format!(
                            "{:.3},{:.3},{:.3}",
                            w[0].target.x, w[0].target.y, w[0].target.z
                        ),
                        format!(
                            "{:.3},{:.3},{:.3}",
                            w[1].target.x, w[1].target.y, w[1].target.z
                        ),
                    )
                })
                .collect()
        };
        assert_eq!(cuts(&tp), cuts(&out), "cut geometry must be identical");
    }

    #[test]
    fn relink_refuses_links_that_leave_the_boundary() {
        use crate::geo::P2;
        use crate::geometry::region_set::RegionSet;
        use crate::polygon::Polygon2;

        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let base = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (_, unbounded) = relink(&tp, &mesh, &index, &tool, &base);
        assert_eq!(
            unbounded.surface_links, 5,
            "control: without a boundary every junction links"
        );

        // A boundary that covers the cut runs but NOT the gaps between
        // them: each 1.5mm junction now crosses excluded territory.
        let mut rings: Vec<Polygon2> = Vec::new();
        let mut x = 1.0;
        for _ in 0..6 {
            rings.push(Polygon2::new(vec![
                P2::new(x - 0.2, -1.0),
                P2::new(x + 4.2, -1.0),
                P2::new(x + 4.2, 1.0),
                P2::new(x - 0.2, 1.0),
            ]));
            x += 4.0 + 1.5;
        }
        let region = RegionSet::new(rings);
        let bounded_params = RelinkParams {
            boundary: Some(&region),
            ..base
        };
        let (_, bounded) = relink(&tp, &mesh, &index, &tool, &bounded_params);
        assert_eq!(
            bounded.surface_links, 0,
            "a link crossing excluded territory is a CUTTING feed over ground \
             the op was confined away from — the selective-finishing gouge \
             class: {bounded:?}"
        );
        assert_eq!(bounded.outside_boundary, 5, "{bounded:?}");
        assert_eq!(bounded.retract_links, 5, "{bounded:?}");
    }

    /// G-LINKVETO: the territory veto exists for SURFACE-RIDING links, which
    /// are cutting feeds. A CEILING link is airborne by construction — it
    /// traverses at `max(surface, standing stock) + clearance` — so an op
    /// whose boundary only confines CUTTING may let it cross excluded
    /// territory. Same fixture as
    /// [`relink_refuses_links_that_leave_the_boundary`]: the only variable
    /// is the ceiling (plus the op prior that goes with it), so the tests
    /// pin that the veto keys on the LINK KIND **and** the op's declared
    /// prior — never on the boundary's presence, and never on the link
    /// shape alone. The third member of the family,
    /// [`an_airborne_link_still_answers_a_territory_boundary`], holds the
    /// prior at `false` with everything else identical. (Measured cost of the
    /// veto firing in ceiling mode on the confined wanaka tier 1: 17,083
    /// intra-node safe-Z retract trips — 363.8 m of rapids for 86.8 m of
    /// cutting.)
    #[test]
    fn a_ceiling_link_may_cross_excluded_territory() {
        use crate::geo::P2;
        use crate::geometry::region_set::RegionSet;
        use crate::polygon::Polygon2;

        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let mut rings: Vec<Polygon2> = Vec::new();
        let mut x = 1.0;
        for _ in 0..6 {
            rings.push(Polygon2::new(vec![
                P2::new(x - 0.2, -1.0),
                P2::new(x + 4.2, -1.0),
                P2::new(x + 4.2, 1.0),
                P2::new(x - 0.2, 1.0),
            ]));
            x += 4.0 + 1.5;
        }
        let region = RegionSet::new(rings);
        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: Some(&region),
            link_ceiling: Some(LinkCeiling {
                stock: None,
                tool_radius: 1.0,
                fallback_top_z: 0.0,
            }),
            flush_ride: false,
            // The op prior half of the exemption. Without it the veto stands
            // even for an airborne link — that is the other sentry,
            // `an_airborne_link_still_answers_a_territory_boundary`.
            airborne_links_may_leave_territory: true,
        };
        let (linked, report) = relink(&tp, &mesh, &index, &tool, &params);
        assert_eq!(
            report.surface_links, 5,
            "airborne links cross excluded territory: {report:?}"
        );
        assert_eq!(report.outside_boundary, 0, "{report:?}");

        // And they really are airborne: every HORIZONTALLY travelling
        // Linking move clears the ceiling's material top by the plunge
        // clearance. The vertical exit and re-entry legs end AT the cut by
        // design, so the XY filter — not a special case — exempts them,
        // same as `island_stay_down_links_o3`.
        let floor = 0.0 + crate::toolpath::PLUNGE_CLEARANCE_MM - 1e-6;
        for w in linked.moves.windows(2) {
            let (prev, mv) = (&w[0], &w[1]);
            let travels_xy =
                (mv.target.x - prev.target.x).hypot(mv.target.y - prev.target.y) > 1e-6;
            if travels_xy
                && mv.intent == crate::toolpath::MoveIntent::Linking
                && !matches!(mv.move_type, crate::toolpath::MoveType::Rapid)
            {
                assert!(
                    mv.target.z >= floor.min(safe_z),
                    "a ceiling link sample at z={} sits below the material \
                     clearance {floor}",
                    mv.target.z
                );
            }
        }
    }

    /// The other half of G-LINKVETO, and the property `project_curve`
    /// depends on: being airborne is NOT on its own a licence to leave the
    /// machining boundary. A boundary can encode a keep-out or a fixture,
    /// and [`LinkCeiling`] reads its clearance from the dexel stock, which
    /// need not model a clamp at all — so an op that has not declared
    /// [`RelinkParams::airborne_links_may_leave_territory`] keeps the veto
    /// even in ceiling mode.
    ///
    /// Same fixture and the same ceiling as
    /// [`a_ceiling_link_may_cross_excluded_territory`]; the ONLY variable is
    /// the flag. Written after the exemption was briefly inferred from
    /// `link_ceiling.is_some()`, which made this refusal — and with it
    /// project_curve's boundary contract — unreachable.
    #[test]
    fn an_airborne_link_still_answers_a_territory_boundary() {
        use crate::geo::P2;
        use crate::geometry::region_set::RegionSet;
        use crate::polygon::Polygon2;

        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        // A boundary that covers the cut runs but NOT the 1.5mm gaps between
        // them, so every junction crosses excluded territory.
        let mut rings: Vec<Polygon2> = Vec::new();
        let mut x = 1.0;
        for _ in 0..6 {
            rings.push(Polygon2::new(vec![
                P2::new(x - 0.2, -1.0),
                P2::new(x + 4.2, -1.0),
                P2::new(x + 4.2, 1.0),
                P2::new(x - 0.2, 1.0),
            ]));
            x += 4.0 + 1.5;
        }
        let region = RegionSet::new(rings);
        let params = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: Some(&region),
            link_ceiling: Some(LinkCeiling {
                stock: None,
                tool_radius: 1.0,
                fallback_top_z: 0.0,
            }),
            flush_ride: false,
            // The whole variable. `true` here is the sibling test.
            airborne_links_may_leave_territory: false,
        };
        let (_, report) = relink(&tp, &mesh, &index, &tool, &params);
        assert!(
            report.outside_boundary > 0,
            "the territory veto must FIRE on an airborne link whose op did \
             not declare the exemption — not merely coincide with a path \
             that had no links to lose: {report:?}"
        );
        assert_eq!(
            report.outside_boundary, 5,
            "all five junctions cross excluded territory: {report:?}"
        );
        assert_eq!(
            report.surface_links, 0,
            "…and nothing may have slipped past it: {report:?}"
        );
        assert_eq!(
            report.retract_links, 5,
            "every refused link falls back to the retract it replaced: \
             {report:?}"
        );
    }

    /// The flush arm of the pencil prior: where the dexel says the stock is
    /// already AT the surface along the hop, a ceiling-mode link rides the
    /// surface — no clearance staple — and, being a CUTTING feed again,
    /// answers to the territory veto like any surface link. Fixture: the
    /// ceiling's fallback top sits far BELOW the mesh, so material never
    /// stands above the surface and every hop reads flush.
    #[test]
    fn a_flush_ceiling_link_rides_the_surface_and_answers_the_veto() {
        use crate::geo::P2;
        use crate::geometry::region_set::RegionSet;
        use crate::polygon::Polygon2;

        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;
        let tp = fragmented_valley_path(6, 4.0, 1.5, safe_z);

        let flush_ceiling = LinkCeiling {
            stock: None,
            tool_radius: 1.0,
            fallback_top_z: -50.0,
        };
        let base = RelinkParams {
            hookup_distance: 3.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: Some(flush_ceiling),
            flush_ride: true,
            // Exemption GRANTED at the op level, so what refuses the bounded
            // arm below can only be the flush link's own cutting shape — the
            // property this test is about.
            airborne_links_may_leave_territory: true,
        };
        let (linked, report) = relink(&tp, &mesh, &index, &tool, &base);
        assert_eq!(report.surface_links, 5, "flush hops link: {report:?}");
        // Surface-riding, not stapled: no horizontally-travelling link
        // sample sits a clearance above the surface (the staple's signature
        // height); the ride stays at cut depth.
        let staple_floor = crate::toolpath::PLUNGE_CLEARANCE_MM - 1e-6;
        for w in linked.moves.windows(2) {
            let (prev, mv) = (&w[0], &w[1]);
            let travels_xy =
                (mv.target.x - prev.target.x).hypot(mv.target.y - prev.target.y) > 1e-6;
            if travels_xy
                && mv.intent == crate::toolpath::MoveIntent::Linking
                && !matches!(mv.move_type, crate::toolpath::MoveType::Rapid)
            {
                assert!(
                    mv.target.z < staple_floor,
                    "a flush link sample at z={} carries the staple shape the \
                     flush arm exists to remove",
                    mv.target.z
                );
            }
        }

        // And the veto is back in force for the flush (cutting) shape: the
        // same excluded-gap boundary that a LIFTED link may cross refuses a
        // flush one.
        let mut rings: Vec<Polygon2> = Vec::new();
        let mut x = 1.0;
        for _ in 0..6 {
            rings.push(Polygon2::new(vec![
                P2::new(x - 0.2, -1.0),
                P2::new(x + 4.2, -1.0),
                P2::new(x + 4.2, 1.0),
                P2::new(x - 0.2, 1.0),
            ]));
            x += 4.0 + 1.5;
        }
        let region = RegionSet::new(rings);
        let bounded = RelinkParams {
            boundary: Some(&region),
            ..base
        };
        let (_, bounded_report) = relink(&tp, &mesh, &index, &tool, &bounded);
        assert_eq!(
            bounded_report.outside_boundary, 5,
            "a flush link is a cutting feed and answers the territory veto: \
             {bounded_report:?}"
        );
    }

    #[test]
    fn relink_reorder_visits_nearest_first() {
        use crate::toolpath::{MoveIntent, Toolpath};
        let mesh = make_v_valley(60.0, 6.0, 0.5, 60, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let safe_z = 20.0;

        // Three runs emitted in a deliberately bad order: near, far, middle.
        let mut tp = Toolpath::new();
        for x in [1.0_f64, 40.0, 20.0] {
            tp.rapid_to_with_intent(P3::new(x, 0.0, safe_z), MoveIntent::Linking);
            tp.feed_to_with_intent(P3::new(x, 0.0, -3.0), 100.0, MoveIntent::EntryPlunge);
            tp.feed_to_with_intent(P3::new(x + 3.0, 0.0, -3.0), 500.0, MoveIntent::FinishingCut);
            tp.rapid_to_with_intent(P3::new(x + 3.0, 0.0, safe_z), MoveIntent::Retract);
        }

        let base = RelinkParams {
            hookup_distance: 0.0, // linking off: isolate the ordering
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: 500.0,
            plunge_rate: 100.0,
            safe_z,
            link_kinematics: None,
            reorder: false,
            boundary: None,
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (kept, _) = relink(&tp, &mesh, &index, &tool, &base);
        let reordered_params = RelinkParams {
            reorder: true,
            ..base
        };
        let (reordered, _) = relink(&tp, &mesh, &index, &tool, &reordered_params);
        assert!(
            rapid_len(&reordered) < rapid_len(&kept),
            "near→far→middle must reorder to near→middle→far: {} -> {}",
            rapid_len(&kept),
            rapid_len(&reordered)
        );
    }

    /// A surface link rides the mesh (finite Z everywhere) when both ends sit on
    /// it, and returns None when the span is off the mesh (caller then retracts).
    #[test]
    fn build_surface_link_follows_surface_and_detects_offmesh() {
        let mesh = make_v_valley(20.0, 6.0, 0.5, 20, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);

        let on = build_surface_link(
            P3::new(5.0, 0.0, 0.0),
            P3::new(9.0, 0.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        let pts = on.unwrap();
        assert!(
            !pts.is_empty(),
            "a 4mm link at 0.5mm spacing has interior points"
        );
        for p in &pts {
            assert!(p.z.is_finite(), "each link point rides the surface");
        }

        let off = build_surface_link(
            P3::new(100.0, 100.0, 0.0),
            P3::new(105.0, 100.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        assert!(off.is_none(), "a link entirely off the mesh must be None");
    }
}
