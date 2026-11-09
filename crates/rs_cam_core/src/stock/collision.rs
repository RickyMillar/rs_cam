//! Tool holder collision detection: warn when the shank, the holder or a
//! hard shoe part would meet the workpiece mesh during a toolpath.
//!
//! P22-0c-5: the mesh check runs THE contact kernel
//! (`stock::contact::zone_top_at`) on the model mesh
//! (`stock::contact::ModelMesh`): each contact zone of the mounted body
//! (`tool::body::MountedBody::zones`) that gives a mesh probe
//! (`ZoneProbe::on_mesh`) is tested at every sample of every cutting move.
//! The mesh top under a zone is the drop of a flat disc of the zone radius.

use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::stock::contact::{ModelMesh, ZoneProbe, zone_top_at};
use crate::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use crate::tool::body::{MountedBody, Zone, ZoneKind};
use crate::toolpath::{MoveType, Toolpath};

/// What the holder/shank collided with — the workpiece mesh, or a
/// workholding fixture. Lets the UI attribute "holder crashed a clamp"
/// rather than only counting it (W0.1 / P6-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionKind {
    /// The workpiece mesh being cut.
    Workpiece,
    /// An obstacle of the setup (K1: a fixture or a keep-out zone),
    /// identified by its name.
    Obstacle { name: String },
    /// H1: the fresh stock box of the setup, which a hard shoe part or a
    /// shelf of the head struck.
    Stock,
}

/// A single collision event.
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    /// Index of the move in the toolpath that caused the collision.
    ///
    /// STK-10: spelled `move_index`, the same name [`RapidCollision`] uses.
    pub move_index: usize,
    /// Position of the tool tip when collision occurs.
    pub position: P3,
    /// How deep the holder penetrates the obstacle (mm, positive = penetration).
    pub penetration_depth: f64,
    /// The zone that collided (P22-0c-5: the zone kind of the body; the
    /// wire names are the old segment names).
    pub segment: ZoneKind,
    /// What was hit — the workpiece mesh or a specific fixture.
    pub kind: CollisionKind,
}

/// Result of a collision check.
#[derive(Debug)]
pub struct CollisionReport {
    /// All detected collision events.
    pub collisions: Vec<CollisionEvent>,
    /// Minimum stickout that would avoid all collisions (mm).
    /// If no collisions, this equals the current stickout.
    pub min_safe_stickout: f64,
}

impl CollisionReport {
    /// True if no collisions were detected.
    pub fn is_clear(&self) -> bool {
        self.collisions.is_empty()
    }
}

/// What a holder/shank collision check knows about one toolpath.
///
/// Three states, because a check that FAILED is not a check that found
/// nothing. `holder_collision_counts` used to answer `0` for all three,
/// so every consumer read a failure as a clean bill of health on the one
/// question that wrecks a machine (CMP-14). Commit `70a3db27` fixed the
/// same shape in the CLI on the audit day; this is the core twin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderCollisionCheck {
    /// The toolpath binds no mesh AND its setup has no enabled fixture —
    /// a 2D operation with nothing around it. The check does not apply.
    /// [`Self::count`] reports `Some(0)`, which is the CLI's rule
    /// (`SessionError::MissingGeometry` is the expected 2D case there):
    /// with no model and no fixture there is no geometry to collide with,
    /// so the count is a true zero rather than a withheld one.
    ///
    /// A mesh-less toolpath whose setup DOES carry a fixture is measured,
    /// not skipped: the fixture half of the check needs no mesh (CMP-14
    /// follow-up).
    NotApplicable,
    /// The check ran and could not finish — a cancellation, or a toolpath
    /// whose tool the project no longer defines. The count is UNKNOWN.
    /// It must never read as zero.
    Failed,
    /// The check ran to the end. `Measured(0)` is measured and clear.
    Measured(usize),
    /// P22-0c-4 (operator ruling Q1): a fresh simulation measured the
    /// shank, the holder and the shoe on the live stock
    /// (`SimulationResult::body_contacts`, the fed contacts). It
    /// supersedes the mesh check, as `tool.flute_reach_within` supersedes
    /// `tool.reach_past_flute`: the live stock holds the model, so a part
    /// that meets the model meets the stock. `MeasuredOnStock(0)` is
    /// measured and clear.
    MeasuredOnStock(usize),
}

impl HolderCollisionCheck {
    /// The count a consumer may publish. `None` means the check failed,
    /// so no count exists. Read [`Self::NotApplicable`] for why a
    /// not-applicable check reports `Some(0)`.
    pub fn count(self) -> Option<usize> {
        match self {
            Self::NotApplicable => Some(0),
            Self::Failed => None,
            Self::Measured(n) | Self::MeasuredOnStock(n) => Some(n),
        }
    }

    /// Collisions this check actually found. A failed check found none,
    /// because it found nothing at all — use [`Self::failed`] to report
    /// the absence.
    pub fn collisions(self) -> usize {
        match self {
            Self::Measured(n) | Self::MeasuredOnStock(n) => n,
            Self::NotApplicable | Self::Failed => 0,
        }
    }

    /// Did the check run and fail?
    pub fn failed(self) -> bool {
        matches!(self, Self::Failed)
    }
}

/// The penetration (mm) above which a body segment counts as a strike.
pub const BODY_STRIKE_THRESHOLD_MM: f64 = 0.01;

/// How far one body segment of the assembly sinks into the mesh with the
/// tool tip at `tip`: the one rule every holder/shank check applies.
///
/// The segment is a flat cylinder of `seg_radius` whose bottom sits
/// `z_offset` above the tip. Its drop-cutter height over the tip's XY,
/// minus that bottom, is the penetration. `None` when no triangle is under
/// the cylinder. A penetration above [`BODY_STRIKE_THRESHOLD_MM`] is a
/// strike.
///
/// [`check_collisions`] calls this per sample and per segment, and
/// [`crate::maps::tier_flute_reach`] calls it per tier-map cell, so the
/// two cannot disagree about what a strike is.
pub fn body_segment_penetration_mm(
    tip: P3,
    z_offset: f64,
    seg_radius: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> Option<f64> {
    ModelMesh { mesh, index }
        .top_in_disc(tip.x, tip.y, seg_radius)
        .map(|top| top - (tip.z + z_offset))
}

/// The mesh probes of `mounted` ([`ZoneProbe::on_mesh`]), from the tip up:
/// the shank, then each hard head band, each only when wider than the
/// cutter envelope.
fn mesh_probes(mounted: &MountedBody) -> Vec<ZoneProbe> {
    let envelope = mounted.body().envelope_radius_mm();
    mounted
        .zones()
        .iter()
        .filter_map(|zone| match zone {
            Zone::Contact(contact) => ZoneProbe::on_mesh(contact, envelope),
            Zone::Cutting(_) => None,
        })
        .collect()
}

/// The collision event of the first probe of `probes` that strikes the
/// mesh with the tip at `tip`, with its penetration: THE kernel
/// ([`zone_top_at`]) gives the mesh top the zone meets, and the strike is
/// a penetration above [`BODY_STRIKE_THRESHOLD_MM`] (the subtraction of
/// [`body_segment_penetration_mm`]).
fn sample_strike(
    probes: &[ZoneProbe],
    solid: &ModelMesh<'_>,
    move_index: usize,
    tip: P3,
) -> Option<(CollisionEvent, f64)> {
    probes.iter().find_map(|probe| {
        let top = zone_top_at(probe, solid, tip)?;
        let penetration = top - (tip.z + probe.bottom_mm);
        (penetration > BODY_STRIKE_THRESHOLD_MM).then_some((
            CollisionEvent {
                move_index,
                position: tip,
                penetration_depth: penetration,
                segment: ZoneKind::Contact(probe.kind),
                kind: CollisionKind::Workpiece,
            },
            penetration,
        ))
    })
}

/// Check a toolpath for holder/shank collisions against the mesh.
///
/// For each cutting move, checks whether the shank or holder cylinder
/// (modeled as a flat endmill at that radius) would contact the mesh
/// surface above the allowed Z.
///
/// `step_mm` controls interpolation along moves: positions are checked
/// every `step_mm` between move endpoints to catch collisions mid-travel.
/// Use 0.0 to check endpoints only (legacy behavior).
pub fn check_collisions(
    toolpath: &Toolpath,
    mounted: &MountedBody,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> CollisionReport {
    check_collisions_interpolated(toolpath, mounted, mesh, index, 0.0)
}

/// Check collisions with interpolated path sampling.
///
/// Samples every `step_mm` along each move (in addition to endpoints)
/// to catch collisions mid-travel on long linear moves.
pub(crate) fn check_collisions_interpolated(
    toolpath: &Toolpath,
    mounted: &MountedBody,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    step_mm: f64,
) -> CollisionReport {
    match check_collisions_interpolated_with_cancel(
        toolpath,
        mounted,
        mesh,
        index,
        step_mm,
        &crate::interrupt::NeverCancel,
    ) {
        Ok(report) => report,
        Err(_) => CollisionReport {
            collisions: Vec::new(),
            min_safe_stickout: mounted.body().holder_face_mm(),
        },
    }
}

/// How many moves the parallel walk checks in one rayon batch. The calling
/// thread polls cancel between batches, because a `&dyn CancelCheck` is not
/// `Sync` and cannot enter a rayon closure (the compiler refuses it). 1024
/// moves is the batch of round 1 (`ops::project_curve`); one move holds
/// many samples, so each batch gives every thread enough work.
#[cfg(feature = "parallel")]
const PARALLEL_MOVE_CHUNK: usize = 1024;

/// [`check_collisions_interpolated`] with a cancel check.
///
/// Perf W3b: with the `parallel` feature the moves run on rayon in batches
/// of [`PARALLEL_MOVE_CHUNK`]. The samples of move `i` read only
/// `moves[i - 1].target` and `moves[i]`, so the moves are independent.
/// Each move gives its events in sample order, and the batches join them
/// in move order, so the report is the same as the serial walk gives,
/// event for event (sentry `the_collision_walk_is_the_same_in_parallel_perf7`).
///
/// Cancel: the parallel walk polls `cancel` on the calling thread before
/// each batch, the serial walk before each move and each sample. A cancel
/// gives `Err(Cancelled)` on both paths.
pub fn check_collisions_interpolated_with_cancel(
    toolpath: &Toolpath,
    mounted: &MountedBody,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    step_mm: f64,
    cancel: &dyn CancelCheck,
) -> Result<CollisionReport, Cancelled> {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;

        let probes = mesh_probes(mounted);
        let move_count = toolpath.moves.len();
        let mut collisions = Vec::new();
        let mut max_extra_stickout_needed = 0.0_f64;
        let mut start = 0;
        while start < move_count {
            check_cancel(cancel)?;
            let end = (start + PARALLEL_MOVE_CHUNK).min(move_count);
            let per_move: Vec<(Vec<CollisionEvent>, f64)> = (start..end)
                .into_par_iter()
                .map(|move_idx| move_collisions(toolpath, move_idx, &probes, mesh, index, step_mm))
                .collect();
            for (events, move_max) in per_move {
                collisions.extend(events);
                // Order-independent: each value is a penetration that
                // passed `> BODY_STRIKE_THRESHOLD_MM`, so it is finite and
                // positive (a NaN fails that test and never gets here).
                // `f64::max` over such values gives the same bits in any
                // order, and it is the same `f64::max` the serial walk uses.
                max_extra_stickout_needed = max_extra_stickout_needed.max(move_max);
            }
            start = end;
        }

        let min_safe_stickout = mounted.body().holder_face_mm() + max_extra_stickout_needed;
        Ok(CollisionReport {
            collisions,
            min_safe_stickout,
        })
    }

    #[cfg(not(feature = "parallel"))]
    {
        let probes = mesh_probes(mounted);
        let solid = ModelMesh { mesh, index };
        let mut collisions = Vec::new();
        let mut max_extra_stickout_needed = 0.0_f64;

        for (move_idx, mv) in toolpath.moves.iter().enumerate() {
            check_cancel(cancel)?;
            // Only check cutting moves (linear), not rapids
            let is_cutting = matches!(
                mv.move_type,
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            );
            if !is_cutting {
                continue;
            }

            // Get previous position for interpolation
            #[expect(clippy::indexing_slicing, reason = "move_idx > 0 checked in condition")]
            let prev = if move_idx > 0 {
                toolpath.moves[move_idx - 1].target
            } else {
                mv.target
            };

            // Generate sample points along this move
            let sample_points = interpolate_samples(prev, mv.target, step_mm);

            for tip in &sample_points {
                check_cancel(cancel)?;
                // The first zone that strikes at this sample is enough; the
                // walk goes on to the next sample.
                if let Some((event, penetration)) = sample_strike(&probes, &solid, move_idx, *tip) {
                    collisions.push(event);
                    max_extra_stickout_needed = max_extra_stickout_needed.max(penetration);
                }
            }
        }

        let min_safe_stickout = mounted.body().holder_face_mm() + max_extra_stickout_needed;

        Ok(CollisionReport {
            collisions,
            min_safe_stickout,
        })
    }
}

/// The events of move `move_idx` of the parallel walk, in sample order, and
/// the largest penetration among them (`0.0` when there is none). The body
/// is the serial walk's body for one move, with no cancel poll.
#[cfg(feature = "parallel")]
fn move_collisions(
    toolpath: &Toolpath,
    move_idx: usize,
    probes: &[ZoneProbe],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    step_mm: f64,
) -> (Vec<CollisionEvent>, f64) {
    let mut events = Vec::new();
    let mut move_max = 0.0_f64;
    let Some(mv) = toolpath.moves.get(move_idx) else {
        return (events, move_max);
    };
    // Only check cutting moves (linear), not rapids
    let is_cutting = matches!(
        mv.move_type,
        MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
    );
    if !is_cutting {
        return (events, move_max);
    }

    // The previous position for interpolation; the first move starts at
    // its own target.
    let prev = move_idx
        .checked_sub(1)
        .and_then(|i| toolpath.moves.get(i))
        .map_or(mv.target, |p| p.target);

    let solid = ModelMesh { mesh, index };
    for tip in &interpolate_samples(prev, mv.target, step_mm) {
        // The first zone that strikes at this sample is enough; the walk
        // goes on to the next sample.
        if let Some((event, penetration)) = sample_strike(probes, &solid, move_idx, *tip) {
            events.push(event);
            move_max = move_max.max(penetration);
        }
    }
    (events, move_max)
}

/// Sample points along a move at `step_mm` spacing (plus endpoints).
/// `step_mm <= 0.01` checks the move endpoint only (legacy behavior).
/// Shared by the workpiece-mesh and fixture-obstacle checks so both
/// sample identically.
fn interpolate_samples(prev: P3, target: P3, step_mm: f64) -> Vec<P3> {
    if step_mm <= 0.01 {
        return vec![target];
    }
    let dx = target.x - prev.x;
    let dy = target.y - prev.y;
    let dz = target.z - prev.z;
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let n_steps = (dist / step_mm).ceil() as usize;
    if n_steps > 1 {
        let mut pts = Vec::with_capacity(n_steps + 1);
        for i in 0..=n_steps {
            let t = i as f64 / n_steps as f64;
            pts.push(P3::new(prev.x + t * dx, prev.y + t * dy, prev.z + t * dz));
        }
        pts
    } else {
        vec![target]
    }
}

/// A rapid move that passes through stock material.
#[derive(Debug, Clone)]
pub struct RapidCollision {
    /// Index of the move in the toolpath.
    pub move_index: usize,
    /// Start position of the rapid.
    pub start: P3,
    /// End position of the rapid.
    pub end: P3,
}

/// One rapid collision in BOTH move frames (G-RAPIDFRAME).
///
/// [`RapidCollision::move_index`] is the toolpath's own move index, and
/// `SimulationResult::rapid_collision_move_indices` holds the run-global
/// index at the same position. Only the global index can find the toolpath.
/// `ProjectEvidence::attributed_rapid_collisions` builds this record through
/// `compute::simulate::locate_global_move`; every consumer reads it.
#[derive(Debug, Clone)]
pub struct AttributedRapidCollision {
    /// The run-global move index.
    pub global_move: usize,
    /// `(toolpath_id, local_move)`: the toolpath that holds `global_move` and
    /// the toolpath's own move index. `None` when no boundary holds the move.
    pub toolpath: Option<(crate::ids::ToolpathId, usize)>,
    /// The collision record. `None` when the caller has only the global
    /// index.
    pub collision: Option<RapidCollision>,
}

/// Check for rapid (G0) moves that pass through remaining stock material.
///
/// **Scope since Phase S2**: this is the pre-pass for entries whose removal is
/// ANALYTIC (drill / alignment-pin drill via `apply_drill_op`) and which
/// therefore never enter a replay walk. Stamped entries are checked by
/// [`RapidClearanceCheck`] inside their own walk, against live stock and with
/// the cutter's profile. Behaviour here is unchanged.
///
/// Samples points along each rapid move and queries the dexel Z-grid to
/// determine whether material exists at that height. The grid passed in
/// is a *snapshot* — it is not updated as the toolpath progresses, so
/// the carve-outs below cover the in-toolpath patterns where the dexel
/// snapshot disagrees with the column's actual cleared depth.
///
/// **Vertical retracts (same XY, Z going up).** After a cutting move the
/// tool retracts from already-machined material — no stock can be present
/// at that dexel column above the move's end Z.
///
/// **F3 — same-XY descent re-entry to a just-cut column.** When a
/// preceding feed move ended at the same XY at a Z below the rapid's
/// end Z, the rapid is descending back into a column the immediately-
/// previous cut already cleared above its end Z. The dexel snapshot
/// still shows full stock there (it's frozen at toolpath-start), but
/// physically that column is air down to at least the prior feed's end
/// Z. Catches the drill peck `feed-down → rapid-up → rapid-down-to-just-
/// above-floor` pattern (every peck after the first emitted a false
/// positive) and the milling lift-and-dive variant.
pub fn check_rapid_collisions_against_stock(
    toolpath: &Toolpath,
    z_grid: &crate::stock::dexel::DexelGrid,
) -> Vec<RapidCollision> {
    let mut collisions = Vec::new();

    #[expect(
        clippy::indexing_slicing,
        reason = "i ranges 1..len, so i and i-1 are always valid"
    )]
    for i in 1..toolpath.moves.len() {
        if !matches!(toolpath.moves[i].move_type, MoveType::Rapid) {
            continue;
        }

        let start = toolpath.moves[i - 1].target;
        let end = toolpath.moves[i].target;

        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dz = end.z - start.z;

        // Skip purely vertical retracts (same XY, Z going up).
        // After a cutting move the tool retracts from already-machined
        // material — no stock can be present at that dexel column.
        let xy_dist_sq = dx * dx + dy * dy;
        if rapid_is_straight_up(start, end) {
            continue;
        }

        // F3 carve-out: pure-Z descent to a column the previous feed
        // already cleared above this rapid's end. Walk back through
        // pure-Z moves to find the most recent feed at this XY; if it
        // ended at a Z below `end.z`, the column is cleared down to at
        // least `end.z` and this rapid can't physically hit anything.
        if dz < 0.0 && xy_dist_sq < SAME_XY_SQ_MM2 {
            let mut k = i;
            let cleared_above = loop {
                if k == 0 {
                    break None;
                }
                k -= 1;
                let m = &toolpath.moves[k];
                let dxk = m.target.x - end.x;
                let dyk = m.target.y - end.y;
                if dxk * dxk + dyk * dyk >= SAME_XY_SQ_MM2 {
                    // Different XY — peck-cycle re-entry doesn't apply.
                    break None;
                }
                if !matches!(m.move_type, MoveType::Rapid) {
                    // First non-rapid encountered at this XY column.
                    // If it ended at or below `end.z`, the column is
                    // cleared down to (at least) the rapid's end —
                    // the descent passes through air the dexel snapshot
                    // doesn't know about.
                    break Some(m.target.z);
                }
            };
            if let Some(prior_feed_end_z) = cleared_above
                && prior_feed_end_z <= end.z
            {
                continue;
            }
        }

        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let n_steps = (dist / 1.0).ceil().max(1.0) as usize;

        let mut hit = false;
        for step in 0..=n_steps {
            let t = step as f64 / n_steps as f64;
            let px = start.x + t * dx;
            let py = start.y + t * dy;
            let pz = start.z + t * dz;

            // Check against actual remaining stock surface at this XY.
            if let Some((row, col)) = z_grid.world_to_cell(px, py)
                && z_grid
                    .top_z_at(row, col)
                    .is_some_and(|stock_top| pz < f64::from(stock_top))
            {
                hit = true;
                break;
            }
        }

        if hit {
            collisions.push(RapidCollision {
                move_index: i,
                start,
                end,
            });
        }
    }

    collisions
}

/// Clearance slack the live rapid check allows on its HIGH channel, expressed
/// in dexel cells.
///
/// [`crate::dexel_stock::TriDexelStock::max_clearance_tip_z_for_profile`] errs
/// HIGH by construction, and its own docs name the three over-reaches that
/// stack into that error:
///
/// * the cell **half-diagonal** (`FRAC_1_SQRT_2`), because the profile is
///   evaluated at the closest point the cell's material could occupy;
/// * the **half-cell dilation** of the probe disc (`0.5`);
/// * one cell of `conservative_top` **sub-cell over-read** (`1.0`) — the
///   channel reports how high material may stand ANYWHERE in the cell, so a
///   cell the cutter grazed reads at its pre-cut height until it is covered
///   completely.
///
/// Total ≈ 2.207 cells. These are counted in cells, but none of them is a Z
/// error proportional to the cell (G-RAPIDPLUNGETOL plan §1): for a flat tool
/// the half-diagonal costs 0 in Z, and the dilation and the over-read cost
/// either 0 or a FULL wall height at the rim. What the constant does suppress
/// is the rim / own-kerf class — material in partly covered cells at or just
/// past the rim. It also blinded the check to a tip up to 2.207 cells into
/// material under the whole footprint, so it applies to the high channel
/// only. The low channel ([`RapidClearanceCheck::strikes`]) reads the cells
/// wholly inside the footprint, errs low, and needs only a float epsilon.
///
/// It scales with the grid, so it is never hardcoded to one resolution —
/// see [`rapid_clearance_tolerance_mm`].
pub(crate) const RAPID_CLEARANCE_TOLERANCE_CELLS: f64 = std::f64::consts::FRAC_1_SQRT_2 + 0.5 + 1.0;

/// [`RAPID_CLEARANCE_TOLERANCE_CELLS`] in millimetres, for a grid of
/// `cell_size`.
pub(crate) fn rapid_clearance_tolerance_mm(cell_size: f64) -> f64 {
    cell_size * RAPID_CLEARANCE_TOLERANCE_CELLS
}

/// Live-stock, profile-aware rapid clearance check (Phase S2).
///
/// [`check_rapid_collisions_against_stock`] asks a **zero-radius point** of a
/// **frozen** pre-op snapshot. That pair is why the measured strike class was
/// invisible: the strikes are 0.5–2.9 mm off-axis (inter-pass crests at rough
/// swath edges), where the cutter's flank meets material the axis never passes
/// over (`planning/rapid_safety_2026-08-28/S1_RESULTS.md` §3).
///
/// This one rides the simulator's own walk instead, so each rapid is evaluated
/// against the stock **as it exists at that moment of playback** — which is
/// what lets the query be a profile-aware disc without over-flagging the rows
/// the same toolpath has already cut. It has no F3-style same-XY walk-back and
/// needs none: the live stock already knows a just-cut column is empty.
///
/// One [`RapidCollision`] per rapid MOVE (first striking sample wins), so the
/// count keeps its established meaning — *rapids that collide*, not strike
/// points.
pub struct RapidClearanceCheck<'a> {
    cutter: &'a dyn crate::tool::MillingCutter,
    /// The envelope of the cutter. The stamps cut with
    /// `tool::body::cut_table`, which has the same heights (the
    /// blade ring changes only which cells a stamp cuts), so the low channel
    /// reads the heights the stamps wrote. A rapid is a collision question:
    /// the core is a solid body that meets wood although it cuts none, so the
    /// probe keeps the full disc (G-SIMCORE, P22-0b).
    lut: RadialProfileLUT,
    hits: Vec<RapidCollision>,
}

/// K5 ReachFlute (feeds contract P7): a fed move where wood stands over the
/// tool tip higher than the flute length, inside the cutter radius, as the
/// stock stands before the move. The part of the tool above the flute
/// meets wood. One per move (the first striking sample).
#[derive(Debug, Clone, PartialEq)]
pub struct FluteOverreach {
    /// Index of the move in its toolpath.
    pub move_index: usize,
    /// The tool tip position at the striking sample (setup frame).
    pub position: P3,
    /// How high the wood inside the cutter radius stands above the tool
    /// tip at that sample (mm). It is more than the flute length.
    pub material_over_tip_mm: f64,
    /// The flute length of the tool (mm).
    pub flute_length_mm: f64,
}

/// The motion of a move that a contact zone meets the stock on
/// (P22-0c-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactMotion {
    /// A fed move (G1, G2, G3).
    Fed,
    /// A rapid (G0).
    Rapid,
}

impl ContactMotion {
    /// The word a finding prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fed => "fed move",
            Self::Rapid => "rapid",
        }
    }
}

/// P22-0c-4 (design section 5.2): a move where a non-cutting zone of the
/// mounted tool above its cutting edge (the shank, a holder band, a hard
/// shoe part) meets the live stock. One per move per zone (the first
/// sample that meets the stock). A fed contact is
/// `project.holder_collision`; a rapid contact is also a struck rapid
/// (`project.rapid_collision`).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyContact {
    /// Index of the move in its toolpath.
    pub move_index: usize,
    /// The tool tip position at the first sample that meets the stock
    /// (emission frame; the findings print the G-code frame).
    pub position: P3,
    /// The zone kind.
    pub zone: crate::tool::body::ContactKind,
    /// The head part name, for a holder or shoe band; `None` for the shank.
    pub name: Option<String>,
    /// The zone bottom over the tool tip (mm).
    pub zone_bottom_mm: f64,
    /// How high the wood inside the zone stands over the tool tip at that
    /// sample (mm). It is more than `zone_bottom_mm`.
    pub material_over_tip_mm: f64,
    /// The motion of the move.
    pub motion: ContactMotion,
}

impl BodyContact {
    /// The zone word and, for a head band, its part name:
    /// `shank`, `holder 'ER20 nut'`.
    #[must_use]
    pub fn zone_text(&self) -> String {
        match &self.name {
            Some(name) => format!("{} '{name}'", self.zone.as_str()),
            None => self.zone.as_str().to_owned(),
        }
    }
}

/// G-CENTRECUT (SAFETY): a fed move where a tool that does not cut at its
/// centre has wood under its centre core that stands higher over the tip
/// than the blade protrusion (G-SIMCORE, P22-0b: any fed move, any op). The
/// core of the tool meets wood that no blade has cut. One per move (the
/// first striking sample).
#[derive(Debug, Clone, PartialEq)]
pub struct NonCentreCuttingDescent {
    /// Index of the move in its toolpath.
    pub move_index: usize,
    /// The tool tip position at the striking sample (setup frame).
    pub position: P3,
    /// How high the wood under the centre core stands above the tool tip
    /// at that sample (mm). It is more than the blade protrusion.
    pub material_over_tip_mm: f64,
    /// The blade protrusion of the tool (mm).
    pub max_plunge_depth_mm: f64,
    /// The descent angle of the move (degrees; 90 for a plunge).
    pub descent_angle_deg: f64,
    /// The dwell window of the core (mm): the core diameter `2 c`. The
    /// dwell rule (`tool::body::CoreZone`) lets a path descend at most the
    /// blade protrusion along any piece of path with an XY diameter of at
    /// most this window (P22-0c-3: the text states this rule, not the
    /// straight-line angle).
    pub dwell_window_mm: f64,
}

/// The XY distance (mm, squared) under which two points are on one
/// column: 0.1 mm of XY, squared. A rising rapid with less XY travel is
/// straight up ([`rapid_is_straight_up`]); the drill pre-pass reads the
/// same column test for its pure-Z moves.
const SAME_XY_SQ_MM2: f64 = 0.01;

/// Is the rapid `start -> end` straight up (it rises, with less than
/// 0.1 mm of XY travel, [`SAME_XY_SQ_MM2`])? Such a rapid is exempt
/// from every rapid check (the envelope arm and the contact zones,
/// P22-0c-4), and this is a proof rather than an assertion: every shipped
/// cutter's `height_at_radius` is non-decreasing in r, so the solid a
/// cutter occupies at a higher tip Z is a strict subset — in the column
/// sense — of the one it occupied below. The same holds for each body zone
/// above it (the shank, the holder, the shoe), which rises with the tip.
/// The tool just came up through this material; it cannot meet anything
/// on the way out.
#[must_use]
pub fn rapid_is_straight_up(start: P3, end: P3) -> bool {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    end.z > start.z && dx * dx + dy * dy < SAME_XY_SQ_MM2
}

/// Float slack of the low channel at a clearance of `low` mm.
///
/// `ray_top` is stored as `f32`, so a cell stamped at `z' + h` reads back
/// within half an `f32` ULP of it (`f32::EPSILON / 2` relative); two
/// `f32::EPSILON` relative covers that rounding four times over, with room
/// for the `f64` arithmetic on top. `max(|low|, 1)` keeps the slack from
/// vanishing near zero Z.
pub(crate) fn low_channel_epsilon_mm(low: f64) -> f64 {
    2.0 * f64::from(f32::EPSILON) * low.abs().max(1.0)
}

impl<'a> RapidClearanceCheck<'a> {
    /// A check that will probe with `cutter`'s profile. P22-0c-3: it holds
    /// only the rapid arm; the contact zones of fed moves are
    /// [`super::contact::StockContactCheck`].
    pub fn new(cutter: &'a dyn crate::tool::MillingCutter) -> Self {
        Self {
            cutter,
            lut: crate::tool::body::envelope_table(cutter, LUT_SAMPLES),
            hits: Vec::new(),
        }
    }

    /// Does this rapid enter material standing in `stock` right now?
    ///
    /// `radius` is the ENVELOPE radius the walk stamps with, so the probe disc
    /// and the removal it is checked against are the same size.
    ///
    /// Sampling matches the pre-pass: every ≤ 1 mm along the segment, both
    /// endpoints included. `None` from the clearance primitive means the disc
    /// is off-grid or the cells lie past the envelope — no constraint at that
    /// sample, the same silence an out-of-grid rapid has always had.
    pub fn strikes(
        &self,
        stock: &crate::dexel_stock::TriDexelStock,
        start: P3,
        end: P3,
        radius: f64,
    ) -> bool {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dz = end.z - start.z;

        let xy_dist_sq = dx * dx + dy * dy;
        if rapid_is_straight_up(start, end) {
            return false;
        }

        let tolerance = rapid_clearance_tolerance_mm(stock.z_grid.cell_size);
        let dist = (xy_dist_sq + dz * dz).sqrt();
        let n_steps = (dist / 1.0).ceil().max(1.0) as usize;
        for step in 0..=n_steps {
            let t = step as f64 / n_steps as f64;
            let px = start.x + t * dx;
            let py = start.y + t * dy;
            let pz = start.z + t * dz;
            // Two channels (G-RAPIDPLUNGETOL). LOW: the cells wholly inside
            // the footprint, erring low — below it the tip is in material,
            // however shallow. HIGH: every visited cell, erring high, less
            // the rim tolerance — the side strikes low cannot see.
            if let Some((low, high)) =
                stock.clearance_bounds_for_profile(px, py, radius, self.cutter, &self.lut)
                && (low.is_some_and(|low| pz < low - low_channel_epsilon_mm(low))
                    || pz < high - tolerance)
            {
                return true;
            }
        }
        false
    }

    /// Record a struck rapid.
    pub fn record(&mut self, move_index: usize, start: P3, end: P3) {
        self.hits.push(RapidCollision {
            move_index,
            start,
            end,
        });
    }

    /// The rapids recorded so far.
    pub fn hits(&self) -> &[RapidCollision] {
        &self.hits
    }

    /// Consume the check and take its findings.
    pub fn into_hits(self) -> Vec<RapidCollision> {
        self.hits
    }
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};

    /// A 6 mm cutter of `flute` mm, a 6 mm shank, a stickout of
    /// `stickout` mm and one 35 mm holder band at the holder face.
    fn mounted(flute: f64, stickout: f64) -> MountedBody {
        crate::tool::ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.0, flute)),
            6.0,
            stickout,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
        .body()
        .mounted(vec![crate::machine::head::HeadBand::holder_at_face(
            35.0, stickout,
        )])
    }

    fn test_assembly() -> MountedBody {
        mounted(25.0, 35.0)
    }

    #[test]
    fn test_stickout() {
        let asm = test_assembly();
        assert!((asm.body().holder_face_mm() - 35.0).abs() < 0.01);
    }

    /// The mesh probes: the shank (no wider than the cutter) gives none;
    /// the holder at the face reads its whole disc of radius 17.5.
    #[test]
    fn test_segments() {
        let probes = mesh_probes(&test_assembly());
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].kind, crate::tool::body::ContactKind::Holder);
        assert!((probes[0].bottom_mm - 35.0).abs() < 0.01);
        assert!((probes[0].outer_mm - 17.5).abs() < 0.01);
        assert_eq!(probes[0].inner_mm, 0.0);
    }

    #[test]
    fn test_no_collision_high_safe_z() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = test_assembly();

        // Toolpath well above the mesh
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.feed_to(P3::new(10.0, 0.0, 50.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 50.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        assert!(
            report.is_clear(),
            "High Z toolpath should have no collisions"
        );
    }

    #[test]
    fn test_collision_detected_low_z() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = mounted(10.0, 15.0);

        // Toolpath at z=0 over the hemisphere center
        // Hemisphere is 20mm tall. With stickout=15, holder bottom at z=15.
        // Holder radius=17.5mm, drop-cutter contacts hemisphere peak at z≈20.
        // Penetration = 20 - 15 = 5mm.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 25.0));
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(5.0, 0.0, 0.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        // With such a short stickout and large holder over a hemisphere,
        // there should be collisions
        assert!(
            !report.is_clear(),
            "Short stickout over hemisphere should detect collisions"
        );
        assert!(
            report.min_safe_stickout > asm.body().holder_face_mm(),
            "Safe stickout should exceed current stickout"
        );
    }

    #[test]
    fn test_interpolated_catches_mid_move() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = mounted(10.0, 15.0);

        // Long move that sweeps over the hemisphere center
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-30.0, 0.0, 25.0));
        tp.feed_to(P3::new(-30.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(30.0, 0.0, 0.0), 1000.0); // sweeps over hemisphere

        // Without interpolation (endpoint only at x=30)
        let report_no_interp = check_collisions(&tp, &asm, &mesh, &index);
        // With interpolation (samples along the 60mm move)
        let report_interp = check_collisions_interpolated(&tp, &asm, &mesh, &index, 2.0);

        // Interpolated should catch more collisions (mid-move over hemisphere peak)
        assert!(
            report_interp.collisions.len() >= report_no_interp.collisions.len(),
            "Interpolation should catch at least as many collisions: {} vs {}",
            report_interp.collisions.len(),
            report_no_interp.collisions.len()
        );
    }

    #[test]
    fn test_no_collision_adequate_stickout() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 5.0);

        // Generous stickout — holder is well above mesh
        let asm = mounted(40.0, 60.0);

        // Toolpath at mesh surface level (z ~ 0)
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 25.0));
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(15.0, 0.0, 0.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        assert!(
            report.is_clear(),
            "Adequate stickout should have no collisions, got {}",
            report.collisions.len()
        );
    }

    #[test]
    fn test_collision_report_safe_stickout() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = mounted(10.0, 15.0);

        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);

        if !report.is_clear() {
            // min_safe_stickout should be >= stickout + max_penetration
            for c in &report.collisions {
                assert!(c.penetration_depth > 0.0, "Penetration should be positive");
            }
        }
    }

    /// Build a fresh (uncarved) dexel grid from a bounding box.
    fn grid_from_bbox(bbox: &crate::geo::BoundingBox3) -> crate::stock::dexel::DexelGrid {
        crate::stock::dexel::DexelGrid::z_grid_from_bounds(bbox, 1.0)
    }

    /// Build a dexel grid and carve away material above `clear_z` everywhere.
    /// Simulates a rough pass that cleared down to `clear_z`.
    fn grid_roughed_to(
        bbox: &crate::geo::BoundingBox3,
        clear_z: f32,
    ) -> crate::stock::dexel::DexelGrid {
        let mut grid = grid_from_bbox(bbox);
        for ray in &mut grid.rays {
            crate::stock::dexel::ray_subtract_above(ray, clear_z);
        }
        grid
    }

    #[test]
    fn test_rapid_above_stock_no_collision() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.rapid_to(P3::new(100.0, 100.0, 50.0));

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert!(
            collisions.is_empty(),
            "Rapids above stock should not collide"
        );
    }

    #[test]
    fn test_rapid_through_stock_detected() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        // Start high, feed down, then rapid across through stock
        tp.rapid_to(P3::new(0.0, 50.0, 50.0));
        tp.feed_to(P3::new(0.0, 50.0, 10.0), 500.0);
        tp.rapid_to(P3::new(80.0, 50.0, 10.0)); // Z=10 is inside stock (top=20)

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert_eq!(collisions.len(), 1, "Should detect one rapid collision");
        assert_eq!(collisions[0].move_index, 2);
    }

    #[test]
    fn test_no_rapids_empty_result() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, 10.0), 1000.0);
        tp.feed_to(P3::new(80.0, 50.0, 10.0), 1000.0);

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert!(collisions.is_empty(), "No rapids means no rapid collisions");
    }

    #[test]
    fn test_vertical_retract_not_flagged() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, 5.0), 1000.0);
        tp.rapid_to(P3::new(50.0, 50.0, 30.0)); // vertical retract

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert!(
            collisions.is_empty(),
            "Vertical retract should not be flagged, got {} collisions",
            collisions.len()
        );
    }

    #[test]
    fn test_diagonal_retract_through_stock_flagged() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, 5.0), 1000.0);
        tp.rapid_to(P3::new(70.0, 50.0, 30.0)); // diagonal through stock

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert_eq!(
            collisions.len(),
            1,
            "Diagonal retract through stock should be flagged"
        );
    }

    #[test]
    fn test_rapid_through_roughed_airspace_not_flagged() {
        // Stock 100x100x20. Roughed down to Z=8 everywhere.
        // A horizontal rapid at Z=10 should be clear (material removed above Z=8).
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_roughed_to(&stock, 8.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 50.0, 10.0));
        tp.rapid_to(P3::new(80.0, 50.0, 10.0)); // Z=10, stock topped at Z=8

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert!(
            collisions.is_empty(),
            "Rapid above roughed surface should not collide, got {}",
            collisions.len()
        );
    }

    #[test]
    fn test_rapid_through_remaining_stock_flagged() {
        // Stock 100x100x20. Roughed down to Z=15.
        // A horizontal rapid at Z=10 should collide (material exists at Z=10-15).
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_roughed_to(&stock, 15.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 50.0, 25.0));
        tp.feed_to(P3::new(0.0, 50.0, 10.0), 500.0);
        tp.rapid_to(P3::new(80.0, 50.0, 10.0)); // Z=10, stock topped at Z=15

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert_eq!(
            collisions.len(),
            1,
            "Rapid below remaining stock surface should be flagged"
        );
    }

    #[test]
    fn test_finish_raster_after_roughing_no_collisions() {
        // Stock 100x100x12, roughed down to Z=7.
        // Finish raster with safe_z=14 — all rapids should be clear.
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 12.0),
        };
        let grid = grid_roughed_to(&stock, 7.0);
        let safe_z = 14.0;
        let mut tp = Toolpath::new();

        tp.rapid_to(P3::new(0.0, 0.0, safe_z));
        tp.feed_to(P3::new(0.0, 0.0, 3.0), 500.0);
        tp.feed_to(P3::new(100.0, 0.0, 5.0), 1000.0);
        tp.rapid_to(P3::new(100.0, 0.0, safe_z)); // vertical retract

        tp.rapid_to(P3::new(100.0, 1.0, safe_z)); // traverse at safe_z
        tp.feed_to(P3::new(100.0, 1.0, 4.0), 500.0);
        tp.feed_to(P3::new(0.0, 1.0, 6.0), 1000.0);
        tp.rapid_to(P3::new(0.0, 1.0, safe_z)); // vertical retract

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert!(
            collisions.is_empty(),
            "Finish raster after roughing should have zero collisions, got {}",
            collisions.len()
        );
    }

    /// F3 — drill peck cycle pattern: feed-down to peck N, rapid-up to
    /// retract_z, rapid-back-down to peck N + small clearance. The
    /// dexel snapshot is frozen at toolpath start (full stock at the
    /// hole's XY column), but physically the previous peck cleared the
    /// column down to peck N's depth, so the rapid-back-down passes
    /// through air — no collision. Pre-F3 this fired one false positive
    /// per peck after the first.
    #[test]
    fn drill_peck_reentry_rapid_does_not_collide() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        let (x, y) = (50.0, 50.0);
        let retract_z = 22.0;
        // Get into position above hole.
        tp.rapid_to(P3::new(x, y, retract_z));
        // Peck 1: feed down 3mm.
        tp.feed_to(P3::new(x, y, 17.0), 300.0);
        // Retract up — vertical retract, already covered by the
        // dz>0 carve-out.
        tp.rapid_to(P3::new(x, y, retract_z));
        // Re-entry rapid down to just-above peck 1's floor.
        // PRE-F3 this fired a false positive.
        tp.rapid_to(P3::new(x, y, 17.5));
        // Peck 2: feed down to 14mm.
        tp.feed_to(P3::new(x, y, 14.0), 300.0);
        // Retract again.
        tp.rapid_to(P3::new(x, y, retract_z));
        // Re-entry rapid down to just-above peck 2's floor.
        tp.rapid_to(P3::new(x, y, 14.5));

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert!(
            collisions.is_empty(),
            "Drill peck re-entry rapids should not flag as collisions; \
             got {} collisions at moves {:?}",
            collisions.len(),
            collisions.iter().map(|c| c.move_index).collect::<Vec<_>>(),
        );
    }

    /// F3 — the carve-out must not weaken the existing
    /// "rapid-down through uncleared stock" detection. A pure-Z descent
    /// to a column where NO prior feed has cleared (e.g. the very first
    /// rapid down to a position) should still flag.
    #[test]
    fn rapid_descent_to_uncleared_column_still_collides() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let grid = grid_from_bbox(&stock);
        let mut tp = Toolpath::new();
        // Rapid down to Z=10 inside stock — no prior feed at this XY.
        tp.rapid_to(P3::new(50.0, 50.0, 30.0));
        tp.rapid_to(P3::new(50.0, 50.0, 10.0));

        let collisions = check_rapid_collisions_against_stock(&tp, &grid);
        assert_eq!(
            collisions.len(),
            1,
            "Pure-Z rapid down to uncleared stock must still collide",
        );
    }
}
