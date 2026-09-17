//! Toolpath dressups — post-processing transforms applied to toolpaths.
//!
//! Dressups modify an existing toolpath without changing the core operation.
//! They compose: you can chain multiple dressups on the same toolpath.
//!
//! - **Ramp entry**: Replace vertical plunges with helical or ramped entry
//! - **Tab/bridge**: Insert material tabs to hold parts during profile cutting
//!
//! The folder holds the other post-generation transforms too: arc fitting,
//! segment conditioning, feed optimisation, feed modulation, rapid
//! ordering and the entry burial audit. The G-code post-processor is
//! `gcode::post`, which is a different layer.

pub mod arcfit;
pub mod condition;
pub mod entry_audit;
pub mod feed_modulation;
pub mod feedopt;
pub mod tsp;

mod air_cut;
mod entry_descent;
mod link;

pub(crate) use entry_descent::{ENTRY_CLEARANCE, emit_helix, emit_ramp};

use crate::dexel_stock::TriDexelStock;
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;
use crate::toolpath::{Move, MoveType, Toolpath};
use crate::trace::toolpath_spans::{AnnotatedToolpath, MoveRemap, Span, SpanKind};
use crate::trace::transform_provenance::{ReconcileSet, Transformed};

use air_cut::{material_above_cutter, sample_is_air_for_tool, swept_path_is_all_air};
use entry_descent::{
    RampFold, collect_following_cut, find_next_xy_direction, fold_walk_budget, upcoming_run,
};
use link::{LINK_Z_MATCH_TOL, bridge_corridor_is_swept};

/// Two Z heights within this many mm are treated as "the same cutting
/// depth" — used to recognize a run of cutting moves at one Z level
/// (tab placement, lead-in/lead-out cut-Z matching, dogbone corner
/// detection). Not a geometry tolerance; a coarse "same level" test.
const Z_LEVEL_EPS_MM: f64 = 0.01;

/// Two XY positions within this many mm are treated as "the tool didn't
/// move horizontally" — used by [`is_plunge`] to distinguish a vertical
/// plunge from a ramped/angled entry.
const XY_STATIONARY_EPS_MM: f64 = 0.01;

/// How close a closing rapid's XY must be to the cut endpoint a lead-out arc
/// departed from before it is treated as "the generator wrote this as a pure
/// vertical lift and the arc made it stale".
///
/// Deliberately tight — the same coarse "same position" scale as
/// [`XY_STATIONARY_EPS_MM`], not a lead-out radius. A rapid one arc radius
/// away is heading somewhere on purpose and must not be rewritten.
const LEAD_OUT_RETRACT_EPS_MM: f64 = 0.01;

/// Reconcile a dressup result against no provenance channel, and hand back
/// the toolpath alone.
///
/// The one door for a caller that holds no [`ReconcileSet`]: the unit tests,
/// the integration tests and the benches. Production code runs every dressup
/// through `compute::execute::apply_dressups`, which owns the real channel
/// set and reconciles each step itself.
///
/// Until CUT-02 each dressup shipped its own `pub` wrapper that did exactly
/// this, and only a test ever called one. Nine wrappers became one helper.
/// Do not add a tenth.
pub fn without_provenance(transformed: Transformed) -> AnnotatedToolpath {
    transformed
        .reconcile(&mut ReconcileSet::empty())
        .into_inner()
}

// ---------------------------------------------------------------------------
// Ramp / Helix entry
// ---------------------------------------------------------------------------

/// Strategy for entering material (replacing straight plunges).
#[derive(Debug, Clone, Copy)]
pub enum EntryStyle {
    /// Linear ramp: plunge at an angle along the next cutting direction.
    /// `max_angle_deg` is the maximum ramp angle from horizontal (e.g., 3.0°).
    Ramp { max_angle_deg: f64 },
    /// Helical entry: spiral down at the plunge point.
    /// `radius` is the helix radius (mm), `pitch` is Z drop per revolution (mm).
    Helix { radius: f64, pitch: f64 },
}

/// Drop-cutter surface context for stock-aware entry moves
/// (G-RAMPTERRAIN, `planning/entry_moves_2026-09-03/`).
///
/// When present, [`emit_ramp`] and [`emit_helix`] lift every leg and
/// turn sample to `max(planned z, cl_z + stock_to_leave)`, and fall
/// back to a straight plunge when the probe loses surface contact.
/// When absent, the entry keeps the legacy straight legs — honest only
/// for operations with no mesh surface (2D prisms), where the legs cut
/// the material between passes by design.
#[derive(Clone, Copy)]
pub struct EntrySurfaceProbe<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    /// The operation's cutter — the CL surface is tool-profile-aware.
    pub cutter: &'a dyn MillingCutter,
    /// The operation's leave allowance. The probe protects
    /// `surface + stock_to_leave`, not the bare model.
    pub stock_to_leave: f64,
    /// What a sample beyond the mesh footprint means for this caller.
    pub off_mesh: OffMeshEntry,
    /// The stock the operation STARTS from, on a rest-driven pass only
    /// (G-ISOCLIPENTRY, 2026-09-09).
    ///
    /// The mesh floor above says how deep an entry may go. This says how
    /// much material it has to get through to arrive there, which the model
    /// surface cannot: on a `FromRemainingStock` pass the ground the upstream
    /// tool could not reach stands ABOVE the mesh. `Some` only where the
    /// caller's operation is rest driven — `session/compute.rs` passes
    /// `gen_initial_stock`, which is `None` on `StockSource::Fresh` — so a
    /// fresh-stock entry keeps the legacy two-leg ramp exactly.
    pub rest_stock: Option<&'a TriDexelStock>,
}

/// Policy for an entry sample beyond the mesh footprint
/// (G-RAMPTERRAIN design amendment, FINDINGS.md 2026-09-03).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OffMeshEntry {
    /// Full-height uncut stock can stand beyond the part footprint.
    /// Give up the shaped entry and plunge at the entry column — the
    /// generator placed that target on the intended surface, so the
    /// plunge is stock-aware by construction. The dressup door
    /// (surface-riding finish operations) uses this.
    PlungeFallback,
    /// Beyond the mesh footprint stands prism stock the operation is
    /// allowed to cut (2.5D roughing): the planned z stands there.
    /// The adaptive3d door uses this — its entry destination is
    /// draped and its descent floor covers uncut columns.
    Unconstrained,
}

impl EntrySurfaceProbe<'_> {
    /// The protected floor at `(x, y)`: drop-cutter CL height plus the
    /// leave allowance. `None` when the cutter has no surface contact
    /// there (off the mesh footprint).
    pub fn floor_z(&self, x: f64, y: f64) -> Option<f64> {
        let cl = point_drop_cutter(x, y, self.mesh, self.index, self.cutter);
        cl.contacted.then_some(cl.z + self.stock_to_leave)
    }
}

/// The safety context every entry emitter consumes.
///
/// One construction site for "all entry moves should be stock aware"
/// (operator ruling, G-RAMPTERRAIN): both doors into the entry
/// emitters — the dressup layer and adaptive3d's direct calls — build
/// one of these instead of passing loose guard values.
#[derive(Clone, Copy)]
pub struct EntrySafety<'a> {
    /// Z of the top of uncut stock in the cutter frame. Keeps the
    /// entry's initial descent rapid above material (UX-dial-in B1).
    pub stock_top: f64,
    /// Drop-cutter surface probe. See [`EntrySurfaceProbe`].
    pub surface: Option<EntrySurfaceProbe<'a>>,
}

/// Replace straight plunges in a toolpath with ramped or helical entries.
///
/// A "plunge" is detected as a feed move that goes from safe_z (or higher)
/// down to cutting depth with no XY movement. The plunge move is replaced
/// in-place by one or more ramp/helix moves; the remap reflects the 1→K
/// expansion. The inserted moves are tagged with [`SpanKind::Entry`] when
/// the input has valid spans.
///
/// `safety` carries the stock-top rapid guard (UX-dial-in B1) and the
/// optional drop-cutter surface probe (G-RAMPTERRAIN) — see
/// [`EntrySafety`].
///
/// Hands back the 1→K plunge expansion under the C1 provenance contract,
/// so channels other than the spans can follow it.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn apply_entry(
    annotated: AnnotatedToolpath,
    style: EntryStyle,
    plunge_rate: f64,
    safety: EntrySafety<'_>,
    tool_radius_mm: f64,
) -> Transformed {
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;

    let mut result = Toolpath::new();
    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> =
        Vec::with_capacity(toolpath.moves.len());
    let mut entry_ranges: Vec<std::ops::Range<usize>> = Vec::new();

    let mut i = 0;
    while i < toolpath.moves.len() {
        let m = &toolpath.moves[i];

        // Detect a plunge: Linear move that goes downward with no XY change
        if let MoveType::Linear { feed_rate } = m.move_type
            && i > 0
            && is_plunge(&toolpath.moves[i - 1], m)
        {
            let entry_start = result.moves.len();
            match style {
                EntryStyle::Ramp { max_angle_deg } => {
                    // Look ahead for the next XY move to determine ramp direction
                    let ramp_dir = find_next_xy_direction(&toolpath.moves, i);
                    // G-RAMPCONTAIN: the following cut, for the ramp to fold
                    // along. `max_angle_deg` bounds the walk, so the scan is
                    // half a ramp length of path and not the whole pass.
                    let want = fold_walk_budget(max_angle_deg);
                    let follow = collect_following_cut(&toolpath.moves, i, want);
                    let closed = follow.len() >= 3
                        && follow.last().is_some_and(|p| {
                            let dx = p.x - m.target.x;
                            let dy = p.y - m.target.y;
                            (dx * dx + dy * dy).sqrt() < XY_STATIONARY_EPS_MM
                        });
                    let fold = RampFold {
                        follow: &follow,
                        closed,
                        min_run_mm: tool_radius_mm.max(1.0),
                    };
                    emit_ramp(
                        &mut result,
                        &toolpath.moves[i - 1].target,
                        &m.target,
                        ramp_dir,
                        max_angle_deg,
                        feed_rate.min(plunge_rate),
                        &safety,
                        Some(&fold),
                    );
                }
                EntryStyle::Helix { radius, pitch } => {
                    emit_helix(
                        &mut result,
                        &toolpath.moves[i - 1].target,
                        &m.target,
                        radius,
                        pitch,
                        feed_rate.min(plunge_rate),
                        &safety,
                    );
                }
            }
            let entry_end = result.moves.len();
            // Defensive: if entry emitted no moves, skip the tagging step.
            if entry_end > entry_start {
                old_to_new.push(Some(entry_start..entry_end));
                entry_ranges.push(entry_start..entry_end);
            } else {
                old_to_new.push(None);
            }
            i += 1;
            continue;
        }

        let new_idx = result.moves.len();
        result.moves.push(m.clone());
        old_to_new.push(Some(new_idx..new_idx + 1));
        i += 1;
    }

    let new_n_moves = result.moves.len();
    let remap = MoveRemap { old_to_new };
    let new_spans = if spans_valid {
        let mut remapped = remap.remap_spans(&spans, new_n_moves);
        for r in entry_ranges {
            remapped.push(Span::new(r.start, r.end, SpanKind::Entry));
        }
        remapped
    } else {
        spans
    };

    Transformed::from_remap(
        AnnotatedToolpath {
            toolpath: result,
            spans: new_spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        },
        remap,
    )
}

// ---------------------------------------------------------------------------
// Entry-descent optimization (P1 W2, reworked)
// ---------------------------------------------------------------------------

/// The stock-aware re-entry approach for a REST-DRIVEN surface-riding pass
/// (G-ISOCLIPENTRY, 2026-09-09).
///
/// `Some` on a `FromRemainingStock` operation whose
/// [`crate::compute::catalog::OperationConfig::entry_probe_leave`] says it
/// rides the model surface; `None` everywhere else, which reproduces the
/// pre-fix emission move for move.
///
/// # What it repairs
///
/// A boundary clip re-enters its region by rapiding to the target's own XY at
/// `safe_z` and feeding straight down
/// (`crate::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance`), and the
/// scallop family opens every non-helical run the same way. On a fresh-stock
/// pass that descent is air until it reaches the model. On a rest-driven pass
/// the material an upstream tool could not reach stands over exactly those
/// points, so the descent is a full-diameter vertical bite into it — measured
/// on the wanaka200 tier-1 islands as a 1.44 mm ball plunge repeated once per
/// clipped ring re-entry.
///
/// The entry moves the dressups build ARE stock aware (G-RAMPTERRAIN), but
/// they are built before the boundary clip and the clip rapids them away, so
/// the door never sees the descent it invents. This is the post-clip door.
pub struct RestEntryRamp {
    /// The radius that actually nestles into the surface — the tip, never the
    /// envelope. Use [`crate::finish::pencil::tip_contact_radius`].
    pub contact_radius_mm: f64,
    /// Feed for the lap moves. The laps are a peripheral cut, so this is the
    /// operation's cutting feed, not its plunge rate.
    pub feed_rate: f64,
    /// Rate for the vertical air descent down to the lap ladder's ceiling.
    pub plunge_rate: f64,
}

/// P1 W2 (reworked): split long plunge-from-safe_z entries by rapiding
/// down to just above the INPUT STOCK's material ceiling first.
///
/// Scans for the pattern `[Rapid to (x,y, safe-ish z)]` -> `[Linear feed
/// tagged EntryPlunge, same XY (within eps), descending]`. For each, the
/// split is DECIDED on the flat-disc material ceiling
/// `stock.max_conservative_top_z_in_disc(x, y, tool_radius)` (falling back to
/// `fresh_stock_top_z` when the disc has no intersecting column, or when
/// there is no stock at all). When `ceiling + PLUNGE_CLEARANCE_MM` sits at
/// least 0.5mm below the rapid's z AND above the plunge target z, the plunge
/// is split: a new `Rapid(Linking)` down to the descent TARGET, then the
/// original `EntryPlunge` feed for the remainder. Never touches plunges
/// whose XY differs from the preceding rapid (not a vertical entry) or
/// whose intent is not `EntryPlunge`.
///
/// # Descent target — profile-aware (B2)
///
/// `tool_radius` is a SEARCH BOUND, not the tool's shape: it is the furthest
/// lateral offset at which material could reach the cutter at all (the
/// envelope radius). Within it the cutter's own profile decides how high
/// material has to stand before it can touch, so the target height is
/// [`crate::dexel_stock::TriDexelStock::max_clearance_tip_z_for_profile`]
/// `+ PLUNGE_CLEARANCE_MM` — the same primitive
/// [`crate::finish::surface_link::LinkCeiling`] adopted for stay-down links (Track A1).
/// The flat disc demanded clearance over ridges a tapered flank physically
/// clears, so descents stopped higher than needed and the surplus was spent
/// as fed air at plunge rate.
///
/// **HEIGHT, not DECISION.** The gate above is evaluated on the flat-disc
/// answer exactly as it was before B2, so the pass fires on precisely the
/// same entries it always did; only the inserted rapid's Z moves, and only
/// ever DOWNWARD (`height_at_radius >= 0` for every profile, so the profile
/// answer is bounded above by the flat one — and it is `min`-clamped to it
/// regardless). The one exception is the near-unreachable case where the
/// profile ceiling falls at or below the plunge's own target: `h(0) = 0`
/// means a plunge that ends in material at its own XY always has a profile
/// ceiling above that column's top, so only a whole-tool-air plunge can
/// reach it, and those are the air-cut filter's business (S3). There the
/// flat target stands rather than emitting a zero-length or inverted feed.
///
/// SAFETY: the profile query errs HIGH by construction (sliver-safe
/// `conservative_top`, half-cell disc dilation, and the profile evaluated at
/// the cell's nearest possible approach), so the inserted rapid is
/// collision-free — unlike any mesh-derived height, which understates
/// remaining stock on `FromRemainingStock` ops (the 151-collision Rivers
/// lesson, `planning/unified_finishing_pass_plan.md` P1 notes). For a FLAT
/// endmill `height_at_radius(r) == Some(0.0)` throughout the envelope, so
/// the emitted toolpath is byte-identical to the pre-B2 one. Pinned by
/// `tests/entry_descent_profile_b2.rs` (byte-identity, the taper relaxation,
/// the non-relaxation where the flank would strike, and a live
/// `RapidClearanceCheck` replay asserting zero strikes).
///
/// # Ramp instead of plunge on a rest-driven pass (G-ISOCLIPENTRY, 2026-09-09)
///
/// With `ramp` set the pass does not stop at lowering the rapid. Below the
/// stock ceiling the descent still had to feed, and on a `FromRemainingStock`
/// surface-riding pass everything below that ceiling is material an upstream
/// tool could not reach — so the fed part was a full-diameter vertical bite,
/// once per entry. It is now bite-budgeted zig-zag laps along the run's own
/// first millimetre or so ([`crate::finish::pencil::plan_entry_ramp`] with
/// `end_at_start`), which is the same manoeuvre and the same construction site
/// the pencil family already used against G-ENTRYLOAD.
///
/// `ramp` is `None` for every caller that is not a rest-driven surface-riding
/// operation, and the plan itself abstains when the run is too short to ramp
/// along or the stock over the window already sits within one bite budget of
/// the finished surface. Both make the emission byte-identical to the pre-fix
/// one, so `tests/entry_descent_profile_b2.rs` and
/// `tests/descent_resolution_stability_am10.rs` hold unchanged.
///
/// One hole is left open deliberately: the ramp is reached only where the
/// split gate fires or a plan exists, and a plunge whose preceding rapid is
/// ALREADY within [`MIN_SPLIT_MM`] of the stock ceiling keeps its descent when
/// no plan is produced. Nothing shipped emits that shape — every generator
/// rapids to `safe_z` first — and the post-simulation `project.entry_load`
/// finding reports it if one ever does.
///
/// This is a thin wrapper over
/// [`optimize_entry_descents_with_provenance`] that discards the
/// provenance mapping — use that function directly when the caller needs
/// to remap spans (e.g. after span construction, mirroring
/// [`crate::geometry::boundary::clip_toolpath_to_boundary_with_provenance`]'s
/// contract).
///
/// Returns the number of entries split (for logging/tests).
pub fn optimize_entry_descents(
    tp: &mut Toolpath,
    stock: Option<&TriDexelStock>,
    fresh_stock_top_z: f64,
    tool_radius: f64,
    cutter: &dyn crate::tool::MillingCutter,
    ramp: Option<&RestEntryRamp>,
) -> usize {
    optimize_entry_descents_with_provenance(tp, stock, fresh_stock_top_z, tool_radius, cutter, ramp)
        .0
}

/// [`optimize_entry_descents`] at the [`AnnotatedToolpath`] level, under the
/// C1 provenance contract: remaps the spans and hands the mapping to every
/// registered channel through [`Transformed::reconcile`].
///
/// This is the shape the pipeline should call. Three call sites used to
/// hand-roll the same three lines — clip/split, `spans.iter().map(remap)`,
/// then remember to poke the semantic trace — and "remember to" is exactly
/// the convention C1 exists to delete. Returns the split count alongside,
/// because callers log it.
///
/// The `if split_count > 0` guard the call sites used to apply is folded in:
/// a zero-split run reports an identity mapping, which is a no-op for every
/// channel and cheaper to state than to branch on.
pub fn optimize_entry_descents_annotated(
    annotated: AnnotatedToolpath,
    stock: Option<&TriDexelStock>,
    fresh_stock_top_z: f64,
    tool_radius: f64,
    cutter: &dyn crate::tool::MillingCutter,
    ramp: Option<&RestEntryRamp>,
) -> (Transformed, usize) {
    let AnnotatedToolpath {
        mut toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;

    let (split_count, mapping) = optimize_entry_descents_with_provenance(
        &mut toolpath,
        stock,
        fresh_stock_top_z,
        tool_radius,
        cutter,
        ramp,
    );

    let spans = if split_count > 0 {
        spans.iter().map(|s| s.remap(&mapping)).collect()
    } else {
        spans
    };

    (
        Transformed::from_mapping(
            AnnotatedToolpath {
                toolpath,
                spans,
                spans_valid,
                planner_engagement,
                rest_grid,
                rest_regions,
            },
            mapping,
        ),
        split_count,
    )
}

/// Same as [`optimize_entry_descents`] but also returns the per-input-move
/// provenance mapping — same shape as
/// [`crate::geometry::boundary::clip_toolpath_to_boundary_with_provenance`]'s second
/// return value (`mapping[i]` is the first output move index produced from
/// input move `i`; `mapping[tp.moves.len()]` is the output move count).
///
/// Prefer [`optimize_entry_descents_annotated`] when spans exist: it applies
/// the mapping to them and to every other registered channel for you.
pub fn optimize_entry_descents_with_provenance(
    tp: &mut Toolpath,
    stock: Option<&TriDexelStock>,
    fresh_stock_top_z: f64,
    tool_radius: f64,
    cutter: &dyn crate::tool::MillingCutter,
    ramp: Option<&RestEntryRamp>,
) -> (usize, Vec<usize>) {
    use crate::toolpath::MoveIntent;

    const MIN_SPLIT_MM: f64 = 0.5;
    const XY_EPS_MM: f64 = 1e-6;

    let moves = std::mem::take(&mut tp.moves);
    // G-ISOCLIPENTRY: the lap window is read off the moves that FOLLOW the
    // plunge, so the run has to be visible before the list is consumed. Two
    // parallel reads, not a clone of the moves.
    let followers: Vec<(P3, bool, MoveIntent)> = moves
        .iter()
        .map(|m| (m.target, m.move_type.is_cutting(), m.intent))
        .collect();
    let mut new_moves = Vec::with_capacity(moves.len());
    let mut mapping = Vec::with_capacity(moves.len() + 1);
    let mut splits = 0usize;
    let mut ramped = 0usize;

    let mut input_index = 0usize;
    let mut iter = moves.into_iter().peekable();
    while let Some(rapid) = iter.next() {
        mapping.push(new_moves.len());
        let rapid_index = input_index;
        input_index += 1;

        let rapid_xy = (rapid.target.x, rapid.target.y);
        let rapid_z = rapid.target.z;
        let is_rapid = rapid.move_type == MoveType::Rapid;

        let split_z = is_rapid
            .then(|| iter.peek())
            .flatten()
            .filter(|plunge| {
                plunge.intent == MoveIntent::EntryPlunge
                    && matches!(plunge.move_type, MoveType::Linear { .. })
                    && (plunge.target.x - rapid_xy.0).abs() < XY_EPS_MM
                    && (plunge.target.y - rapid_xy.1).abs() < XY_EPS_MM
                    && plunge.target.z < rapid_z - 1e-6
            })
            .and_then(|plunge| {
                // A/M10 — resolution honesty, by model rather than by pad.
                //
                // This used to read `max_top_z_in_disc` (cell-centre columns)
                // and add `2 * cell_size` of slack, because the snapshot's
                // disc-max under-reads material the grid smoothed away: the
                // SAME generated chain measured 0/15/20 rapid collisions at
                // 0.5/0.25/0.1 mm (TP15 RCA, 2026-07-13). A cell-scaled pad
                // is the right shape for a ridge CREST, whose error scales
                // with the cell — and no shape at all for an uncut rib
                // narrower than one cell, which stands at whatever the
                // original stock was, an unbounded distance above the
                // blended reading. Padding could only ever chase that class.
                //
                // `max_conservative_top_z_in_disc` answers the question this
                // code is actually asking — *how high can material be
                // ANYWHERE under the tool* — and answers it as an upper
                // bound that refining the grid can only lower. So the pad is
                // gone: the bound is already conservative, and stacking a
                // heuristic on top of a bound just buys air time back.
                //
                // B2 (2026-08-29, PROGRAMME.md Track B) — *how high can
                // material be* is still the right question for the DECISION,
                // and the wrong one for the HEIGHT. The flat disc models the
                // cutter as a cylinder of `tool_radius`; past its tip a real
                // cutter RISES, so material at lateral offset `r` can only
                // strike it if it stands more than `height_at_radius(r)`
                // above the tip. Reading the disc flat demanded clearance
                // over ridges a tapered flank physically clears, and every
                // millimetre of that surplus was then spent as fed air at
                // plunge rate. Same defect Track A1 fixed for stay-down
                // links, same primitive.
                //
                // The split gate stays on the flat answer verbatim, so the
                // pass fires on exactly the entries it always did; only the
                // target moves, and only downward.
                let flat_ceiling = stock
                    .and_then(|s| {
                        s.max_conservative_top_z_in_disc(rapid_xy.0, rapid_xy.1, tool_radius)
                    })
                    .unwrap_or(fresh_stock_top_z);
                let z_flat = flat_ceiling + crate::toolpath::PLUNGE_CLEARANCE_MM;
                (z_flat <= rapid_z - MIN_SPLIT_MM && z_flat > plunge.target.z).then(|| {
                    // `min(z_flat)` is belt-and-braces: the profile query is
                    // bounded above by the flat one whenever both have an
                    // answer, and this also covers the case where the
                    // profile query abstains (every visited cell past the
                    // envelope) while the flat one did not.
                    let z_profile = stock
                        .and_then(|s| {
                            s.max_clearance_tip_z_for_profile(
                                rapid_xy.0,
                                rapid_xy.1,
                                tool_radius,
                                cutter,
                            )
                        })
                        .map_or(z_flat, |c| {
                            (c + crate::toolpath::PLUNGE_CLEARANCE_MM).min(z_flat)
                        });
                    // Never rapid to or below the plunge's own target: that
                    // would emit a zero-length or inverted feed. Near-dead
                    // arm (`h(0) = 0` puts the profile ceiling above the
                    // column the plunge lands in whenever it lands in
                    // material at all), and the flat target — shipped
                    // behaviour — is what stands there.
                    if z_profile > plunge.target.z {
                        z_profile
                    } else {
                        z_flat
                    }
                })
            });

        // G-ISOCLIPENTRY: the same peek, asked a second question. `ramp` is
        // `Some` only on a rest-driven surface-riding pass, so every other
        // caller emits move for move what it always did.
        let ramp_plan = ramp.zip(stock).and_then(|(cfg, snapshot)| {
            let plunge = is_rapid.then(|| iter.peek()).flatten()?;
            if plunge.intent != MoveIntent::EntryPlunge
                || !matches!(plunge.move_type, MoveType::Linear { .. })
                || (plunge.target.x - rapid_xy.0).abs() >= XY_EPS_MM
                || (plunge.target.y - rapid_xy.1).abs() >= XY_EPS_MM
                || plunge.target.z >= rapid_z - 1e-6
            {
                return None;
            }
            // A generator that already ramps its own entry (pencil) is left
            // alone. Its laps follow the descent, so planning over them would
            // stack a second ladder on the first and read the first ladder's
            // own Z range as standing material.
            if followers
                .get(rapid_index + 2)
                .is_some_and(|&(_, _, intent)| intent == MoveIntent::EntryRamp)
            {
                return None;
            }
            let run = upcoming_run(&followers, rapid_index + 1);
            crate::finish::pencil::plan_entry_ramp(
                &run,
                snapshot,
                cfg.contact_radius_mm,
                rapid_z,
                true,
            )
        });

        new_moves.push(rapid);

        if split_z.is_some() || ramp_plan.is_some() {
            if let Some(z) = split_z {
                new_moves.push(Move {
                    target: P3::new(rapid_xy.0, rapid_xy.1, z),
                    move_type: MoveType::Rapid,
                    intent: MoveIntent::Linking,
                });
                splits += 1;
            }
            mapping.push(new_moves.len());
            // Both arms only fire when `iter.peek()` above was `Some`, so the
            // plunge move is guaranteed to exist here.
            if let Some(plunge) = iter.next() {
                input_index += 1;
                match (&ramp_plan, ramp) {
                    (Some(plan), Some(cfg)) => {
                        // The vertical part is AIR by construction: the ladder
                        // starts at the conservative stock ceiling read over
                        // the whole lap window, and `max_conservative_top_z_in_disc`
                        // may only ever err high.
                        let from_z = split_z.unwrap_or(rapid_z);
                        if plan.air_descent_z < from_z - 1e-9 {
                            new_moves.push(Move {
                                target: P3::new(rapid_xy.0, rapid_xy.1, plan.air_descent_z),
                                move_type: MoveType::Linear {
                                    feed_rate: cfg.plunge_rate,
                                },
                                intent: MoveIntent::EntryPlunge,
                            });
                        }
                        for point in &plan.points {
                            new_moves.push(Move {
                                target: *point,
                                move_type: MoveType::Linear {
                                    feed_rate: cfg.feed_rate,
                                },
                                intent: MoveIntent::EntryRamp,
                            });
                        }
                        // `end_at_start` puts the last lap point on the run's
                        // first point at its own finished Z — the plunge's
                        // target. The plunge is therefore consumed, not
                        // emitted; `mapping` already points this input move at
                        // the first lap move, so the no-drop contract holds.
                        ramped += 1;
                    }
                    _ => new_moves.push(plunge),
                }
            }
        }
    }
    mapping.push(new_moves.len());
    if ramped > 0 {
        tracing::debug!(
            ramped,
            splits,
            "rest-driven entry descents ramped instead of plunged"
        );
    }
    tp.moves = new_moves;
    (splits, mapping)
}

fn is_plunge(prev: &Move, current: &Move) -> bool {
    if let MoveType::Linear { .. } = current.move_type {
        let dz = current.target.z - prev.target.z;
        let pdx = current.target.x - prev.target.x;
        let pdy = current.target.y - prev.target.y;
        let dxy = (pdx * pdx + pdy * pdy).sqrt();
        // Downward move with negligible XY movement
        dz < -0.1 && dxy < XY_STATIONARY_EPS_MM
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Tab / Bridge dressup
// ---------------------------------------------------------------------------

/// A tab (bridge) that holds the part to the stock during profile cutting.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Position along the polygon perimeter (0.0 to 1.0, fractional).
    pub position: f64,
    /// Width of the tab in mm.
    pub width: f64,
    /// Height of the tab in mm (how far above cut_depth the tab rises).
    pub height: f64,
}

/// Insert holding tabs into a profile toolpath.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Tabs create sharp rectangular bridges: the cutter steps up to tab height,
/// traverses at that height, then steps back down. This leaves material
/// bridges that hold the part to the stock.
///
/// Tab positions are interpolated along cutting segments, so tabs appear
/// at the correct location even when move endpoints are sparse.
///
/// # Move intent (CUT-08)
///
/// Every rewritten segment keeps the [`MoveIntent`] of the move it replaces,
/// so a tabbed profile stays visible to each intent-keyed reader — the
/// spacing instrument (`metrology/spacing.rs`) skips a move that is not
/// `FinishingCut`. The two fed descents back to `cut_depth` after a tab are
/// [`MoveIntent::EntryPlunge`]: they enter material vertically, which is what
/// that tag means everywhere else in the crate.
///
/// The fed lift onto a tab keeps the cut intent. It removes material, and a
/// `Retract`-tagged feed is forbidden — see
/// `tests/retract_intent_move_type_census_w6.rs`.
pub fn apply_tabs(toolpath: Toolpath, tabs: &[Tab], cut_depth: f64) -> Toolpath {
    use crate::toolpath::MoveIntent;

    if tabs.is_empty() {
        return toolpath;
    }

    // Collect cutting move indices at cut_depth
    let cutting_indices: Vec<usize> = toolpath
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            matches!(m.move_type, MoveType::Linear { .. })
                && (m.target.z - cut_depth).abs() < Z_LEVEL_EPS_MM
        })
        .map(|(i, _)| i)
        .collect();

    if cutting_indices.len() < 2 {
        return toolpath;
    }

    // Compute cumulative distance at each cutting move endpoint
    let mut cum_dist = vec![0.0_f64];
    for i in 1..cutting_indices.len() {
        let prev = &toolpath.moves[cutting_indices[i - 1]].target;
        let curr = &toolpath.moves[cutting_indices[i]].target;
        let ddx = curr.x - prev.x;
        let ddy = curr.y - prev.y;
        let d = (ddx * ddx + ddy * ddy).sqrt();
        // SAFETY: cum_dist is initialized with [0.0] and only grows
        #[allow(clippy::expect_used)]
        cum_dist.push(cum_dist.last().expect("cum_dist starts with [0.0]") + d);
    }

    let total_dist = *cum_dist.last().unwrap_or(&0.0);
    if total_dist < 1e-6 {
        return toolpath;
    }

    // Build sorted tab boundary events as absolute distances
    struct TabZone {
        start_dist: f64,
        end_dist: f64,
        tab_z: f64,
    }
    let tab_zones: Vec<TabZone> = tabs
        .iter()
        .map(|tab| {
            let center_dist = tab.position * total_dist;
            let half_w = tab.width / 2.0;
            TabZone {
                start_dist: (center_dist - half_w).max(0.0),
                end_dist: (center_dist + half_w).min(total_dist),
                tab_z: cut_depth + tab.height,
            }
        })
        .collect();

    let tab_z_at_dist = |dist: f64| -> Option<f64> {
        tab_zones
            .iter()
            .find(|tz| dist >= tz.start_dist && dist <= tz.end_dist)
            .map(|tz| tz.tab_z)
    };

    // Walk through toolpath, interpolating tab boundaries along cutting segments
    let mut result = Toolpath::new();
    let mut in_tab = false;

    for (i, m) in toolpath.moves.iter().enumerate() {
        let cut_pos = cutting_indices.iter().position(|&ci| ci == i);

        if let Some(cp) = cut_pos {
            if cp == 0 {
                // First cutting move — just emit it
                result.moves.push(m.clone());
                in_tab = tab_z_at_dist(0.0).is_some();
                continue;
            }

            let feed_rate = match m.move_type {
                MoveType::Linear { feed_rate } => feed_rate,
                _ => 1000.0,
            };

            let seg_start_dist = cum_dist[cp - 1];
            let seg_end_dist = cum_dist[cp];
            let prev_target = &toolpath.moves[cutting_indices[cp - 1]].target;
            let curr_target = &m.target;
            let seg_len = seg_end_dist - seg_start_dist;

            if seg_len < 1e-10 {
                result.moves.push(m.clone());
                continue;
            }

            // Collect all tab boundary crossings within this segment
            let mut events: Vec<(f64, bool)> = Vec::new(); // (dist, is_entry)
            for tz in &tab_zones {
                if tz.start_dist > seg_start_dist && tz.start_dist < seg_end_dist {
                    events.push((tz.start_dist, true));
                }
                if tz.end_dist > seg_start_dist && tz.end_dist < seg_end_dist {
                    events.push((tz.end_dist, false));
                }
            }
            events.sort_by(|a, b| a.0.total_cmp(&b.0));

            if events.is_empty() {
                // No boundary crossings — whole segment is in or out
                let mid_dist = (seg_start_dist + seg_end_dist) / 2.0;
                if let Some(tab_z) = tab_z_at_dist(mid_dist) {
                    if !in_tab {
                        // Entered tab zone before this segment
                        if let Some(last) = result.moves.last() {
                            result.feed_to_with_intent(
                                P3::new(last.target.x, last.target.y, tab_z),
                                feed_rate,
                                m.intent,
                            );
                        }
                        in_tab = true;
                    }
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                        m.intent,
                    );
                } else {
                    if in_tab {
                        if let Some(last) = result.moves.last() {
                            result.feed_to_with_intent(
                                P3::new(last.target.x, last.target.y, cut_depth),
                                feed_rate,
                                MoveIntent::EntryPlunge,
                            );
                        }
                        in_tab = false;
                    }
                    result.moves.push(m.clone());
                }
            } else {
                // Process boundary crossings — split segment at each event
                let mut last_dist = seg_start_dist;

                for (event_dist, is_entry) in &events {
                    let t = (*event_dist - seg_start_dist) / seg_len;
                    let split_x = prev_target.x + t * (curr_target.x - prev_target.x);
                    let split_y = prev_target.y + t * (curr_target.y - prev_target.y);

                    if *is_entry {
                        // Emit segment up to tab entry at cut_depth
                        if !in_tab {
                            result.feed_to_with_intent(
                                P3::new(split_x, split_y, cut_depth),
                                feed_rate,
                                m.intent,
                            );
                        }
                        // Step up
                        let tab_z = tab_z_at_dist(*event_dist + 0.01).unwrap_or(cut_depth + 2.0);
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, tab_z),
                            feed_rate,
                            m.intent,
                        );
                        in_tab = true;
                    } else {
                        // Emit segment up to tab exit at tab height
                        let tab_z = tab_z_at_dist(last_dist + 0.01).unwrap_or(cut_depth + 2.0);
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, tab_z),
                            feed_rate,
                            m.intent,
                        );
                        // Step down
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, cut_depth),
                            feed_rate,
                            MoveIntent::EntryPlunge,
                        );
                        in_tab = false;
                    }
                    last_dist = *event_dist;
                }

                // Emit remainder of segment after last event
                if in_tab {
                    let tab_z = tab_z_at_dist(last_dist + 0.01).unwrap_or(cut_depth + 2.0);
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                        m.intent,
                    );
                } else {
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, cut_depth),
                        feed_rate,
                        m.intent,
                    );
                }
            }
        } else {
            // Non-cutting move — pass through unchanged
            in_tab = false;
            result.moves.push(m.clone());
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Lead-in / Lead-out dressup
// ---------------------------------------------------------------------------

/// Lift the lead plunge target and the lead arc samples to the probe
/// floor (G-RAMPTERRAIN S2). Returns `false` when the probe loses
/// surface contact at any point — the caller then skips the lead
/// insertion and keeps the generator's original moves.
fn lift_lead_points(
    probe: &EntrySurfaceProbe<'_>,
    plunge_target: &mut P3,
    arc_pts: &mut [P3],
) -> bool {
    let Some(floor) = probe.floor_z(plunge_target.x, plunge_target.y) else {
        return false;
    };
    plunge_target.z = plunge_target.z.max(floor);
    lift_arc_points(probe, arc_pts)
}

/// Lift arc samples to the probe floor; `false` on lost contact.
fn lift_arc_points(probe: &EntrySurfaceProbe<'_>, arc_pts: &mut [P3]) -> bool {
    for q in arc_pts {
        let Some(floor) = probe.floor_z(q.x, q.y) else {
            return false;
        };
        q.z = q.z.max(floor);
    }
    true
}

/// Insert arc lead-in and lead-out moves at the start/end of cutting passes.
///
/// A "cutting pass" is a sequence of feed moves at the same Z bounded by
/// rapids or plunges. The lead-in is a quarter-circle arc that approaches
/// the first cut point tangentially (avoiding a witness mark from a direct
/// plunge). The lead-out is a matching arc departing the last cut point.
///
/// `radius` is the arc radius in mm (typically 1-3mm or ~half the tool radius).
///
/// Lead-in moves replace the original plunge: the old plunge index maps to
/// the inserted prefix arc range, all tagged [`SpanKind::Entry`]. Lead-out
/// moves are emitted *after* the original cut endpoint: the old cut-end move
/// remaps to the union of itself plus the appended arcs, with the suffix
/// tagged [`SpanKind::LeadOut`].
///
/// Hands back the arc insertions under the C1 provenance contract, so
/// channels other than the spans can follow them.
///
/// # Feed overrides (F-040)
///
/// `lead_in_feed_rate` — when `Some`, lead-in arc moves use this feed
/// (typically slower than cutting feed for a softer entry). When `None`,
/// inherits the cut-pass's `feed_rate` (pre-F-040 behaviour).
///
/// `lead_out_feed_rate` — same fallback semantics; typically faster than
/// cutting feed for chip-clear on exit.
///
/// Lead-in moves are tagged [`MoveIntent::LeadIn`] and lead-out moves
/// [`MoveIntent::LeadOut`] so the F-039 modulator (and future analyses)
/// can treat them as user-tuned rather than modulating them.
///
/// `surface` (G-RAMPTERRAIN S2): with a probe, the lead plunge target
/// and every lead arc sample lift to `max(cut_z, floor)` — the lead
/// stays tangential in XY and follows the surface in Z. If the probe
/// loses contact at any lead sample, the insertion for that pass is
/// SKIPPED and the generator's original plunge / retract stays. When
/// no sample lifts, the emitted moves are byte-identical to the
/// probe-less output.
///
/// # `retract_z` (G-ISOCLIPRAPID, 2026-09-09)
///
/// The height of the lead-in's PRE-POSITION rapid — the one move of the
/// manoeuvre that travels in XY, and therefore the only one that has to
/// clear the stock.
///
/// It used to be read from the move before the plunge
/// (`moves[i - 1].target.z`, documented as "the preceding rapid's Z").
/// That move is not always a rapid and not always safe: a stepped pass
/// and a lead-out / lead-in chain both put a CUTTING move there, and an
/// earlier dressup can lower or consume the generator's retract. The
/// lead-in then traversed the work at cutting depth. Measured on the
/// wanaka200 tier-1 islands (`T2_r20_raster_then_r10_iso_islands.toml`,
/// 0.2 mm): one rapid through stock at move 68538, 4.7 mm of lateral
/// travel with both ends at the same sub-stock height.
///
/// `Some(z)` uses the operation's retract plane, which is clear of the
/// stock by construction ([`crate::compute::config::effective_safe_z`]
/// floors it at `stock_top + SAFE_Z_CLEARANCE_MM`). It is the only
/// height available at this layer that is: the surface probe answers
/// about the MODEL, which on a rest-driven pass sits BELOW the material
/// — the same lesson G-ISOCLIPENTRY records.
///
/// `None` means the caller holds no retract plane (the tests that build a
/// synthetic toolpath): the inherited height stands, which is what this
/// dressup always did. The one production caller,
/// `compute::execute::apply_dressups`, passes its own `safe_z`.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn apply_lead_in_out(
    annotated: AnnotatedToolpath,
    radius: f64,
    lead_in_feed_rate: Option<f64>,
    lead_out_feed_rate: Option<f64>,
    surface: Option<&EntrySurfaceProbe<'_>>,
    retract_z: Option<f64>,
) -> Transformed {
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;
    let mut result = Toolpath::new();
    let moves = &toolpath.moves;
    if moves.is_empty() {
        return Transformed::index_preserving(AnnotatedToolpath {
            toolpath: result,
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        });
    }

    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(moves.len());
    let mut entry_ranges: Vec<std::ops::Range<usize>> = Vec::new();
    let mut leadout_ranges: Vec<std::ops::Range<usize>> = Vec::new();

    // Where the last emitted lead-out arc left the tool, and the cut endpoint
    // it departed from. Consumed by the very next move — see the retract
    // rewrite below.
    let mut lead_out_landed: Option<(P3, P3)> = None;

    let mut i = 0;
    while i < moves.len() {
        // ── The retract must retract from where the tool IS ──────────────
        //
        // A lead-out arc is INSERTED between the last cut and the pass's
        // closing rapid, so a rapid that was emitted at the cut endpoint —
        // a pure vertical lift when the generator wrote it — becomes a
        // diagonal rapid travelling BACKWARDS across the surface it has just
        // finished, while climbing to safe Z. Over a feature taller than the
        // climb at that XY, that is a rapid through stock.
        //
        // Found 2026-08-06 by F2: sharpening scallop's coverage guard moved
        // the last fragment of the corrugated A/M7 fixture from a corner to
        // the plate centre, and the same closing retract that had always
        // been diagonal started clipping a ridge — `scallop_intra_pass_
        // relink_am7::no_new_collisions_at_the_finest_resolution`, 0 -> 1
        // rapid collision at 0.1 mm. The retract was latent, not new: the
        // relinker emits it at the FRAGMENT exit (`surface_link.rs`'s
        // trailing `rapid_to_with_intent`), which this dressup then makes
        // stale by appending the arc.
        //
        // The repair is narrow on purpose: only a rapid that lands exactly
        // on the cut endpoint the arc just departed from is rewritten, and
        // only in XY, to the arc's own endpoint. That turns it back into the
        // pure vertical lift it was written as. A rapid heading anywhere
        // else is a real traverse and is left alone.
        if let Some((cut_end, arc_end)) = lead_out_landed.take()
            && moves[i].move_type == MoveType::Rapid
            && moves[i].target.z > cut_end.z
            && (moves[i].target.x - cut_end.x).abs() < LEAD_OUT_RETRACT_EPS_MM
            && (moves[i].target.y - cut_end.y).abs() < LEAD_OUT_RETRACT_EPS_MM
        {
            let new_idx = result.moves.len();
            let mut lifted = moves[i].clone();
            lifted.target = P3::new(arc_end.x, arc_end.y, moves[i].target.z);
            result.moves.push(lifted);
            old_to_new.push(Some(new_idx..new_idx + 1));
            i += 1;
            continue;
        }

        // Detect the start of a cutting pass: a plunge (downward feed) followed
        // by horizontal feed moves at the same Z.
        if i > 0 && is_plunge(&moves[i - 1], &moves[i]) {
            let cut_z = moves[i].target.z;
            let plunge_end = moves[i].target;

            // Find next horizontal cutting move to determine lead-in direction
            if let Some(first_cut_idx) = (i + 1..moves.len()).find(|&j| {
                matches!(moves[j].move_type, MoveType::Linear { .. })
                    && (moves[j].target.z - cut_z).abs() < Z_LEVEL_EPS_MM
            }) {
                let cut_dir_x = moves[first_cut_idx].target.x - plunge_end.x;
                let cut_dir_y = moves[first_cut_idx].target.y - plunge_end.y;
                let cut_dir_len = (cut_dir_x * cut_dir_x + cut_dir_y * cut_dir_y).sqrt();

                if cut_dir_len > 0.1 {
                    let ux = cut_dir_x / cut_dir_len;
                    let uy = cut_dir_y / cut_dir_len;

                    // Lead-in: approach from the side, quarter-circle arc
                    // Start point is offset perpendicular to cut direction
                    let perp_x = -uy;
                    let perp_y = ux;
                    let lead_start = P3::new(
                        plunge_end.x + perp_x * radius - ux * radius,
                        plunge_end.y + perp_y * radius - uy * radius,
                        cut_z,
                    );

                    let plunge_rate = match moves[i].move_type {
                        MoveType::Linear { feed_rate } => feed_rate,
                        _ => 500.0,
                    };
                    // F-040: lead-in uses the dressup's override feed if set,
                    // otherwise falls back to the plunge feed (pre-F-040 default).
                    let li_feed = lead_in_feed_rate.unwrap_or(plunge_rate);

                    // F-040a: classic lead-in geometry — pre-position rapid
                    // over `lead_start` at safe-Z (read from the preceding
                    // rapid's Z), then pure-Z plunge straight down at
                    // `plunge_rate`, THEN tangent arc into the cut at
                    // `li_feed`. Pre-F-040a behaviour was a diagonal feed
                    // from `(cut_start.xy, safe_z)` to `(lead_start.xy, cut_z)`
                    // followed by the arc — geometrically a slanted plunge,
                    // not a tangent entry. The classic shape matches
                    // production CAM (Fusion HSM / Mastercam) and is what
                    // operators expect when they enable lead_in_out.
                    // G-ISOCLIPRAPID: the pre-position rapid travels in XY,
                    // so it belongs at the operation's retract plane. The
                    // inherited height is kept only where the caller holds no
                    // plane to offer, and it is never LOWERED below one the
                    // caller does offer.
                    let safe_z = match retract_z {
                        Some(plane) => plane.max(moves[i - 1].target.z),
                        None => moves[i - 1].target.z,
                    };

                    // Plan the whole lead before emitting anything, so
                    // the surface probe can lift or veto it
                    // (G-RAMPTERRAIN S2).
                    // Arc from lead_start to plunge_end (quarter circle)
                    let arc_steps = 8;
                    let mut arc_pts: Vec<P3> = Vec::with_capacity(arc_steps);
                    for s in 1..=arc_steps {
                        let t = s as f64 / arc_steps as f64;
                        let angle = std::f64::consts::FRAC_PI_2 * t;
                        let (sin_a, cos_a) = angle.sin_cos();
                        let ax = plunge_end.x + perp_x * radius * (1.0 - sin_a)
                            - ux * radius * (1.0 - cos_a);
                        let ay = plunge_end.y + perp_y * radius * (1.0 - sin_a)
                            - uy * radius * (1.0 - cos_a);
                        arc_pts.push(P3::new(ax, ay, cut_z));
                    }
                    let mut plunge_target = lead_start;
                    let lead_ok = match surface {
                        None => true,
                        Some(probe) => lift_lead_points(probe, &mut plunge_target, &mut arc_pts),
                    };

                    if lead_ok {
                        let entry_start = result.moves.len();
                        // Move 0 (G-ISOCLIPRAPID): LIFT before traversing.
                        // Where the tool is still down in the cut, going
                        // straight to `(lead_start.xy, safe_z)` is a rising
                        // DIAGONAL out of the material — safe once the post
                        // splits it (`gcode::program_builder::push_rapid`
                        // retracts first on a rising rapid) but a strike to
                        // every reader of the IR, including
                        // `collision::RapidClearanceCheck`, whose pure-vertical
                        // exemption a diagonal does not get. Emitting the lift
                        // here makes the stored motion say what the machine
                        // does. Skipped when the tool already stands at or
                        // above `safe_z`, so nothing moves where nothing has to.
                        let here = moves[i - 1].target;
                        if safe_z > here.z + 1e-9 {
                            result.rapid_to_with_intent(
                                P3::new(here.x, here.y, safe_z),
                                crate::toolpath::MoveIntent::Retract,
                            );
                        }
                        // Move 1: pre-position rapid at safe_z above lead_start.
                        result.rapid_to_with_intent(
                            P3::new(lead_start.x, lead_start.y, safe_z),
                            crate::toolpath::MoveIntent::LeadIn,
                        );
                        // Move 2: pure-Z plunge down to the lifted target.
                        // Tagged as EntryPlunge so engagement metrics +
                        // modulation treat it as a plunge, not a lead-in arc.
                        result.feed_to_with_intent(
                            plunge_target,
                            plunge_rate,
                            crate::toolpath::MoveIntent::EntryPlunge,
                        );
                        for q in arc_pts {
                            result.feed_to_with_intent(
                                q,
                                li_feed,
                                crate::toolpath::MoveIntent::LeadIn,
                            );
                        }
                        let entry_end = result.moves.len();
                        if entry_end > entry_start {
                            old_to_new.push(Some(entry_start..entry_end));
                            entry_ranges.push(entry_start..entry_end);
                        } else {
                            old_to_new.push(None);
                        }

                        i += 1;
                        continue;
                    }
                    // Probe lost surface contact along the lead: keep the
                    // generator's original plunge (fall through to the
                    // plain copy below).
                }
            }
        }

        // Detect end of a cutting pass: feed at cut_z followed by retract (rapid up)
        if i + 1 < moves.len()
            && matches!(moves[i].move_type, MoveType::Linear { .. })
            && moves[i + 1].move_type == MoveType::Rapid
            && moves[i + 1].target.z > moves[i].target.z + 1.0
        {
            let cut_z = moves[i].target.z;
            let cut_end = moves[i].target;

            // Find the direction of the last cutting segment
            if i > 0 {
                let prev = moves[i - 1].target;
                let dir_x = cut_end.x - prev.x;
                let dir_y = cut_end.y - prev.y;
                let dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();

                if dir_len > 0.1 && (prev.z - cut_z).abs() < Z_LEVEL_EPS_MM {
                    let ux = dir_x / dir_len;
                    let uy = dir_y / dir_len;
                    let perp_x = -uy;
                    let perp_y = ux;

                    let cut_feed_rate = match moves[i].move_type {
                        MoveType::Linear { feed_rate } => feed_rate,
                        _ => 1000.0,
                    };
                    // F-040: lead-out uses the dressup's override feed if set,
                    // otherwise falls back to the cut-pass's feed rate.
                    let lo_feed = lead_out_feed_rate.unwrap_or(cut_feed_rate);

                    // Plan the lead-out arc before emitting anything,
                    // so the surface probe can lift or veto it
                    // (G-RAMPTERRAIN S2).
                    let arc_steps = 8;
                    let mut lo_pts: Vec<P3> = Vec::with_capacity(arc_steps);
                    for s in 1..=arc_steps {
                        let t = s as f64 / arc_steps as f64;
                        let angle = std::f64::consts::FRAC_PI_2 * t;
                        let (sin_a, cos_a) = angle.sin_cos();
                        let ax = cut_end.x + ux * radius * sin_a + perp_x * radius * (1.0 - cos_a);
                        let ay = cut_end.y + uy * radius * sin_a + perp_y * radius * (1.0 - cos_a);
                        lo_pts.push(P3::new(ax, ay, cut_z));
                    }
                    let lead_out_ok = match surface {
                        None => true,
                        Some(probe) => lift_arc_points(probe, &mut lo_pts),
                    };
                    if !lead_out_ok {
                        // Probe lost surface contact along the lead-out:
                        // keep the generator's original retract (fall
                        // through to the plain copy below).
                        let new_idx = result.moves.len();
                        result.moves.push(moves[i].clone());
                        old_to_new.push(Some(new_idx..new_idx + 1));
                        i += 1;
                        continue;
                    }

                    // Emit the original cut endpoint
                    let cut_idx = result.moves.len();
                    result.moves.push(moves[i].clone());

                    // Lead-out: quarter-circle arc departing tangentially
                    let lo_start = result.moves.len();
                    for q in lo_pts {
                        result.feed_to_with_intent(
                            q,
                            lo_feed,
                            crate::toolpath::MoveIntent::LeadOut,
                        );
                    }
                    let lo_end = result.moves.len();
                    // Remember where the arc left the tool, so a closing
                    // rapid still aimed at `cut_end` can be lifted from HERE
                    // instead of travelling back across the finished surface.
                    if let Some(last) = result.moves.last() {
                        lead_out_landed = Some((cut_end, last.target));
                    }
                    // The old cut-end move covers cut_idx..lo_end (itself plus
                    // the appended lead-out arc).
                    old_to_new.push(Some(cut_idx..lo_end));
                    if lo_end > lo_start {
                        leadout_ranges.push(lo_start..lo_end);
                    }

                    i += 1;
                    continue;
                }
            }
        }

        let new_idx = result.moves.len();
        result.moves.push(moves[i].clone());
        old_to_new.push(Some(new_idx..new_idx + 1));
        i += 1;
    }

    let new_n_moves = result.moves.len();
    let remap = MoveRemap { old_to_new };
    let new_spans = if spans_valid {
        let mut remapped = remap.remap_spans(&spans, new_n_moves);
        for r in entry_ranges {
            remapped.push(Span::new(r.start, r.end, SpanKind::Entry));
        }
        for r in leadout_ranges {
            remapped.push(Span::new(r.start, r.end, SpanKind::LeadOut));
        }
        remapped
    } else {
        spans
    };

    Transformed::from_remap(
        AnnotatedToolpath {
            toolpath: result,
            spans: new_spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        },
        remap,
    )
}

// ---------------------------------------------------------------------------
// Dogbone / overcut dressup
// ---------------------------------------------------------------------------

/// Insert dogbone overcuts at inside corners of a toolpath.
///
/// At each corner sharper than `max_angle_deg`, a small extension is cut
/// along the corner bisector so that a mating part with a sharp corner can
/// fit into the CNC-cut pocket. The overcut distance is `tool_radius`,
/// creating a clearance notch at each inside corner.
///
/// Only operates on consecutive linear feed moves at the same Z.
///
/// The corner move at old index `i` remaps to the union of itself plus the
/// (overcut, return) pair appended after it. Each inserted overcut+return
/// pair is tagged with [`SpanKind::DressupArtifact`] (label `"dogbone"`).
///
/// Hands back the overcut insertions under the C1 provenance contract, so
/// channels other than the spans can follow them.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn apply_dogbones(
    annotated: AnnotatedToolpath,
    tool_radius: f64,
    max_angle_deg: f64,
) -> Transformed {
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;
    let max_angle_rad = max_angle_deg.to_radians();
    let mut result = Toolpath::new();

    let moves = &toolpath.moves;
    if moves.len() < 3 {
        // Pass-through with whatever spans were provided.
        return Transformed::index_preserving(AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        });
    }

    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(moves.len());
    let mut dogbone_ranges: Vec<std::ops::Range<usize>> = Vec::new();

    // First move (always emitted as-is).
    let first_idx = result.moves.len();
    result.moves.push(moves[0].clone());
    old_to_new.push(Some(first_idx..first_idx + 1));

    for i in 1..moves.len() - 1 {
        let corner_idx = result.moves.len();
        result.moves.push(moves[i].clone());

        // Only process consecutive linear feed moves at the same Z
        let is_linear = |m: &Move| matches!(m.move_type, MoveType::Linear { .. });
        let mut emitted_overcut = false;

        if is_linear(&moves[i - 1]) && is_linear(&moves[i]) && is_linear(&moves[i + 1]) {
            let a = moves[i - 1].target;
            let b = moves[i].target;
            let c = moves[i + 1].target;

            // Must be at same Z (cutting depth)
            if (a.z - b.z).abs() <= Z_LEVEL_EPS_MM && (b.z - c.z).abs() <= Z_LEVEL_EPS_MM {
                let v1x = b.x - a.x;
                let v1y = b.y - a.y;
                let v2x = c.x - b.x;
                let v2y = c.y - b.y;
                let len1 = (v1x * v1x + v1y * v1y).sqrt();
                let len2 = (v2x * v2x + v2y * v2y).sqrt();

                if len1 >= 1e-10 && len2 >= 1e-10 {
                    let u1x = v1x / len1;
                    let u1y = v1y / len1;
                    let u2x = v2x / len2;
                    let u2y = v2y / len2;

                    let dot = u1x * u2x + u1y * u2y;
                    let angle = dot.clamp(-1.0, 1.0).acos();

                    if angle >= (std::f64::consts::PI - max_angle_rad) {
                        let bx = -u1x + u2x;
                        let by = -u1y + u2y;
                        let blen = (bx * bx + by * by).sqrt();

                        if blen >= 1e-10 {
                            let dx = -(bx / blen);
                            let dy = -(by / blen);

                            let feed_rate = match moves[i].move_type {
                                MoveType::Linear { feed_rate } => feed_rate,
                                _ => 1000.0,
                            };

                            let overcut_x = b.x + dx * tool_radius;
                            let overcut_y = b.y + dy * tool_radius;
                            let overcut_start = result.moves.len();
                            result.feed_to(P3::new(overcut_x, overcut_y, b.z), feed_rate);
                            result.feed_to(b, feed_rate);
                            let overcut_end = result.moves.len();
                            dogbone_ranges.push(overcut_start..overcut_end);
                            emitted_overcut = true;
                        }
                    }
                }
            }
        }

        // Old corner move at index i remaps to corner_idx, optionally extended
        // through the appended overcut pair.
        let new_end = if emitted_overcut {
            result.moves.len()
        } else {
            corner_idx + 1
        };
        old_to_new.push(Some(corner_idx..new_end));
    }

    // Last move (always emitted as-is).
    let last_old = moves.len() - 1;
    let last_new = result.moves.len();
    result.moves.push(moves[last_old].clone());
    old_to_new.push(Some(last_new..last_new + 1));

    let new_n_moves = result.moves.len();
    let remap = MoveRemap { old_to_new };
    let new_spans = if spans_valid {
        let mut remapped = remap.remap_spans(&spans, new_n_moves);
        for r in dogbone_ranges {
            remapped
                .push(Span::new(r.start, r.end, SpanKind::DressupArtifact).with_label("dogbone"));
        }
        remapped
    } else {
        spans
    };

    Transformed::from_remap(
        AnnotatedToolpath {
            toolpath: result,
            spans: new_spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        },
        remap,
    )
}

// ---------------------------------------------------------------------------
// Link-vs-Retract dressup
// ---------------------------------------------------------------------------

/// Parameters for the link-move optimization.
pub struct LinkMoveParams {
    /// Maximum XY distance between passes to replace retract with direct feed.
    /// Default: 3× tool_diameter.
    pub max_link_distance: f64,
    /// Feed rate for link moves (mm/min).
    pub link_feed_rate: f64,
    /// Z threshold: moves to Z at or above this are considered rapids/retracts.
    pub safe_z_threshold: f64,
    /// Cutter radius (mm), used by [`bridge_corridor_is_swept`] to decide
    /// whether a candidate link bridge crosses only ground this toolpath has
    /// already cut. See that function's doc comment for why this replaced
    /// the earlier distance/Z-only guard.
    pub tool_radius: f64,
}

/// Replace short retract→rapid→plunge sequences with direct feed moves.
///
/// Detects 3-move windows of (retract to safe_z, rapid reposition, plunge to cut_z)
/// where the XY distance is short, and replaces them with a single feed move.
///
/// Safety rules:
/// - Never links the first entry (tool hasn't cut yet)
/// - Only links when cut Z before and after are within 0.1mm (same depth level)
/// - max_link_distance caps risk
/// - **Never links across a `RapidOrderBarrier` or `DepthPass` boundary** — the
///   wanaka safety guarantee. If the linked window would erase a barrier, the
///   3-move sequence is preserved as-is.
/// - **Never emits a bridge whose straight-line corridor crosses uncut
///   ground** — see [`bridge_corridor_is_swept`]. This is the fix for a
///   measured defect (Face/Inlay/VCarve/discrete-Scallop gouges); see that
///   function's doc comment and `tests/capability_link_moves_safety.rs`.
///
/// Spans on the input are remapped through the transform, and a `LinkBridge`
/// span is appended for each inserted bridge. `spans_valid` is preserved.
///
/// Hands back the retract-triple → bridge collapse under the C1 provenance
/// contract, so channels other than the spans can follow it.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn apply_link_moves(annotated: AnnotatedToolpath, params: &LinkMoveParams) -> Transformed {
    // Barriers we must not collapse across. A barrier at index `b` sits before
    // moves[b]; collapsing the window (i, i+1, i+2) into one bridge erases the
    // gap between i and i+3 — so any barrier at i+1 or i+2 must block the link.
    // (A barrier at i is *before* the link and is preserved by the remap.)
    let barriers: std::collections::BTreeSet<usize> = if annotated.spans_valid {
        annotated.rapid_order_barriers().into_iter().collect()
    } else {
        std::collections::BTreeSet::new()
    };

    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;
    let moves = &toolpath.moves;
    if moves.len() < 4 {
        return Transformed::index_preserving(AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        });
    }

    let mut result = Toolpath::new();
    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(moves.len());
    let mut bridge_positions: Vec<usize> = Vec::new();

    let mut i = 0;
    let mut has_cut = false;

    while i < moves.len() {
        let m = &moves[i];

        if !has_cut {
            if matches!(m.move_type, MoveType::Linear { .. })
                && m.target.z < params.safe_z_threshold - 1.0
            {
                has_cut = true;
            }
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new.push(Some(new_idx..new_idx + 1));
            i += 1;
            continue;
        }

        let blocked_by_barrier = barriers.contains(&(i + 1)) || barriers.contains(&(i + 2));

        if !blocked_by_barrier
            && i + 2 < moves.len()
            && m.move_type == MoveType::Rapid
            && m.target.z >= params.safe_z_threshold - 0.1
            && moves[i + 1].move_type == MoveType::Rapid
            && moves[i + 1].target.z >= params.safe_z_threshold - 0.1
            && matches!(moves[i + 2].move_type, MoveType::Linear { .. })
            && moves[i + 2].target.z < params.safe_z_threshold - 1.0
        {
            let plunge_target = moves[i + 2].target;

            let prev_cut_z = result
                .moves
                .iter()
                .rev()
                .find(|mv| {
                    matches!(mv.move_type, MoveType::Linear { .. })
                        && mv.target.z < params.safe_z_threshold - 1.0
                })
                .map(|mv| mv.target.z);

            if let Some(prev_z) = prev_cut_z
                && (prev_z - plunge_target.z).abs() < LINK_Z_MATCH_TOL
                && let Some(prev) = result.moves.last().map(|mv| mv.target)
            {
                let dx = moves[i + 1].target.x - prev.x;
                let dy = moves[i + 1].target.y - prev.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < params.max_link_distance {
                    // GOUGE FIX: don't just check distance/Z — verify the
                    // straight bridge crosses only ground this toolpath has
                    // already cut. See `bridge_corridor_is_swept`'s doc
                    // comment for the four measured gouges this closes
                    // (Face 9.21mm, Inlay 8.21mm, VCarve 5.78mm, discrete
                    // Scallop 9.71mm) and why `prior_stock` can't be used
                    // for this check instead.
                    if bridge_corridor_is_swept(
                        &result.moves,
                        prev,
                        plunge_target,
                        params.tool_radius,
                        LINK_Z_MATCH_TOL,
                    ) {
                        let bridge_idx = result.moves.len();
                        result.feed_to(plunge_target, params.link_feed_rate);
                        bridge_positions.push(bridge_idx);
                        let r = bridge_idx..bridge_idx + 1;
                        old_to_new.push(Some(r.clone()));
                        old_to_new.push(Some(r.clone()));
                        old_to_new.push(Some(r));
                        i += 3;
                        continue;
                    }
                    tracing::debug!(
                        from_x = prev.x,
                        from_y = prev.y,
                        from_z = prev.z,
                        to_x = plunge_target.x,
                        to_y = plunge_target.y,
                        to_z = plunge_target.z,
                        tool_radius = params.tool_radius,
                        "apply_link_moves: refusing link bridge — corridor is not \
                         fully covered by already-cut material at this Z; keeping \
                         the retract/rapid/plunge triple instead"
                    );
                }
            }
        }

        let new_idx = result.moves.len();
        result.moves.push(m.clone());
        old_to_new.push(Some(new_idx..new_idx + 1));
        i += 1;
    }

    let new_n_moves = result.moves.len();
    let remap = MoveRemap { old_to_new };
    let new_spans = if spans_valid {
        let mut remapped = remap.remap_spans(&spans, new_n_moves);
        for pos in bridge_positions {
            remapped.push(Span::new(pos, pos + 1, SpanKind::LinkBridge));
        }
        remapped
    } else {
        spans
    };

    Transformed::from_remap(
        AnnotatedToolpath {
            toolpath: result,
            spans: new_spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        },
        remap,
    )
}

/// Generate evenly-spaced tabs around a perimeter.
pub fn even_tabs(count: usize, width: f64, height: f64) -> Vec<Tab> {
    (0..count)
        .map(|i| Tab {
            position: i as f64 / count as f64,
            width,
            height,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Air-cut filter dressup
// ---------------------------------------------------------------------------

/// A4: how deep a toolpath's CUTTING geometry gets into a reference stock,
/// and how many positions that verdict rests on.
///
/// Report-only. Nothing gates on it, and the operation is not refused —
/// see [`reference_engagement_of_cutting_moves`].
///
/// Stays `pub`: `reference_engagement_of_cutting_moves` returns it, so a
/// crate-private form raises `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceEngagement {
    /// Deepest the tip reached below the reference surface, mm. `<= 0` means
    /// it never got under the surface at all, so the pass can remove nothing.
    pub deepest_mm: f64,
    /// Positions sampled. `0` = nothing was measured (no cutting moves, or
    /// none of them landed on the grid) — which is NOT a zero depth, and
    /// callers must not read it as one.
    pub sampled_positions: usize,
}

/// Walk every cutting move of `toolpath` against `stock` and report the
/// deepest the tip got below its surface.
///
/// Sampled along the swept path at the stock grid's own cell size, exactly
/// as [`swept_path_is_all_air`] does and for the same reason: a move whose
/// ENDS are both above the surface can still plough through it in the
/// middle, and an endpoint-only reading is biased in one direction (always
/// toward "nothing here").
///
/// The tip column only, not a disc under the whole cutter. That is a stated
/// limit, not an oversight: material inside the cutter's radius sits above
/// the tip by the tool's own profile, so a disc-max reads positive on any
/// curved surface and the measure would never be able to say "nothing". The
/// price is that a pass which removes material ONLY under its flank —
/// nothing a 3-axis surface-following pass does — reads as zero.
#[must_use]
pub fn reference_engagement_of_cutting_moves(
    toolpath: &Toolpath,
    stock: &TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
) -> ReferenceEngagement {
    let step = stock.z_grid.cell_size.max(1.0e-6);
    let mut deepest = f64::NEG_INFINITY;
    let mut sampled = 0usize;
    let mut arc_buf: Vec<P3> = Vec::new();

    let sample = |p: &P3, deepest: &mut f64, sampled: &mut usize| {
        if let Some(depth) = material_above_cutter(stock, cutter, p.x, p.y, p.z) {
            *sampled += 1;
            if depth > *deepest {
                *deepest = depth;
            }
        }
    };

    for (i, m) in toolpath.moves.iter().enumerate() {
        if m.move_type == MoveType::Rapid {
            continue;
        }
        let prev = i
            .checked_sub(1)
            .and_then(|k| toolpath.moves.get(k))
            .map(|p| p.target);
        let Some(prev) = prev else {
            sample(&m.target, &mut deepest, &mut sampled);
            continue;
        };
        match m.move_type {
            MoveType::ArcCW { i: ci, j: cj, .. } | MoveType::ArcCCW { i: ci, j: cj, .. } => {
                let clockwise = matches!(m.move_type, MoveType::ArcCW { .. });
                crate::geometry::arc_util::linearize_arc_into(
                    &mut arc_buf,
                    prev,
                    m.target,
                    ci,
                    cj,
                    clockwise,
                    step,
                );
                for p in &arc_buf {
                    sample(p, &mut deepest, &mut sampled);
                }
            }
            _ => {
                let (dx, dy, dz) = (
                    m.target.x - prev.x,
                    m.target.y - prev.y,
                    m.target.z - prev.z,
                );
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                let steps = (len / step).ceil().max(1.0) as usize;
                for k in 0..=steps {
                    let t = k as f64 / steps as f64;
                    sample(
                        &P3::new(prev.x + dx * t, prev.y + dy * t, prev.z + dz * t),
                        &mut deepest,
                        &mut sampled,
                    );
                }
            }
        }
    }

    ReferenceEngagement {
        deepest_mm: if sampled == 0 { 0.0 } else { deepest },
        sampled_positions: sampled,
    }
}

/// Should a run of in-air cutting moves be replaced by a retract bridge?
///
/// MEASURED CONTEXT (2026-08-03, `planning/unified_v3_design.md` §10): the
/// filter used to bridge EVERY air run regardless of length. Each bridge is
/// a retract to `safe_z`, a traverse, and a descent — on wanaka ×2 roughly
/// 35 mm of travel — so skipping a 2 mm sliver of air costs ~17× the
/// distance it saves. The unified rest-clearer emitted 1 634 fragments and
/// this filter shattered them into 15 373, adding ~13 700 such bridges and
/// almost all of the op's 539 m of rapid travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AirBridgePolicy {
    /// Bridge every air run, however short. Pre-2026-08 behaviour.
    #[default]
    Always,
    /// Bridge only when the retract round trip is SHORTER than the air path
    /// it replaces.
    ///
    /// Deliberately compares raw distance rather than time, which makes the
    /// rule conservative in one direction only: rapids are never slower than
    /// cutting feeds, so a bridge up to `rapid/feed` times longer than the
    /// air path could still win on the clock and this rule declines it. It
    /// catches every pathological case without needing a machine envelope
    /// threaded into the dressup pipeline; if the refused middle band turns
    /// out to matter, the principled successor is
    /// `machine_kinematics::retract_link_time`, which is what
    /// `pencil::emit_paths` and the unified router already use for the same
    /// bridge-vs-stay-down question.
    ShorterThanAirPath,
}

/// Remove cutting moves that pass through empty stock (no remaining material).
///
/// For each cutting move, checks whether material exists anywhere along the
/// swept path in the prior stock — see [`swept_path_is_all_air`]. Only moves
/// that are in air for their WHOLE length become rapids, so a move that
/// contacts material at any point is preserved, entire.
///
/// # The cutter is not optional (S3)
///
/// Each sample is judged for the whole tool by
/// [`sample_is_air_for_tool`]: cheap centerline test first, envelope-disc
/// profile clearance to confirm any air verdict. Until 2026-08-28 the
/// parameter in `cutter`'s place was a `tool_radius: f64` documented as
/// "reserved for future per-cell radius checks" and never read, so the
/// classifier was a zero-radius point probe. That is the emitter S1 measured:
/// a fed `EntryPlunge` whose exact-XY column reads clear but whose flank
/// stands inside an off-axis crest was reclassified all-air, dropped, and
/// replaced below by a rapid descending to the resume Z — 982 rapid descents
/// into standing hardwood on one shipped job
/// (`planning/rapid_safety_2026-08-28/S1_RESULTS.md`, S3).
///
/// The all-or-nothing whole-move rule is what turns that classifier fix into
/// a safe path: a plunge whose tail strikes crest material stays fed for its
/// whole length. `session::compute`'s `optimize_entry_descents` then re-splits
/// the airborne top of such a plunge against its own envelope-disc ceiling.
///
/// Span behavior: dropped air moves remap to `None`. Each inserted
/// retract/rapid/plunge that bridges across a dropped run is tagged with
/// [`SpanKind::LinkBridge`] (these inserts serve the same role as link
/// bridges and should not block downstream link/TSP passes).
///
/// # Provenance
///
/// This is the one dressup that DELETES moves, so its provenance is the one
/// that makes channels unlink — see
/// [`crate::trace::semantic_trace::ToolpathSemanticItem::move_end`].
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn filter_air_cuts(
    annotated: AnnotatedToolpath,
    prior_stock: &TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
    safe_z: f64,
    tolerance: f64,
    policy: AirBridgePolicy,
) -> Transformed {
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;
    let moves = &toolpath.moves;
    if moves.is_empty() {
        return Transformed::index_preserving(AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        });
    }

    // Phase 1: classify each move as "in air" or not.
    let mut air_flags: Vec<bool> = Vec::with_capacity(moves.len());

    let mut arc_buf: Vec<P3> = Vec::new();
    for (i, m) in moves.iter().enumerate() {
        if m.move_type == MoveType::Rapid {
            air_flags.push(false);
            continue;
        }

        let Some(prev) = i
            .checked_sub(1)
            .and_then(|k| moves.get(k))
            .map(|p| p.target)
        else {
            // No predecessor: only the target is knowable.
            air_flags.push(sample_is_air_for_tool(
                prior_stock,
                cutter,
                m.target.x,
                m.target.y,
                m.target.z,
                tolerance,
            ));
            continue;
        };

        air_flags.push(swept_path_is_all_air(
            prior_stock,
            cutter,
            prev,
            m,
            tolerance,
            &mut arc_buf,
        ));
    }

    // Phase 1b: under `ShorterThanAirPath`, veto the bridges that cost more
    // travel than the air they skip. A vetoed run's moves are un-flagged, so
    // phase 2 emits them verbatim — the tool simply cuts through the sliver
    // of air rather than climbing to `safe_z` and back for it.
    if policy == AirBridgePolicy::ShorterThanAirPath {
        let dist =
            |a: P3, b: P3| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        let mut i = 0usize;
        while i < air_flags.len() {
            if !air_flags.get(i).copied().unwrap_or(false) {
                i += 1;
                continue;
            }
            let run_start = i;
            while air_flags.get(i).copied().unwrap_or(false) {
                i += 1;
            }
            let run_end = i; // exclusive

            // Where the tool is when the run begins, and where it must be
            // when the run ends — the bridge's two endpoints.
            let Some(from) = run_start
                .checked_sub(1)
                .and_then(|k| moves.get(k))
                .map(|m| m.target)
            else {
                continue; // run starts the toolpath: nothing to bridge from
            };
            let Some(to) = run_end
                .checked_sub(1)
                .and_then(|k| moves.get(k))
                .map(|m| m.target)
            else {
                continue;
            };

            let mut air_len = 0.0f64;
            let mut prev = from;
            for k in run_start..run_end {
                if let Some(m) = moves.get(k) {
                    air_len += dist(prev, m.target);
                    prev = m.target;
                }
            }
            // The bridge phase 2 would emit: up to safe_z, across, back down.
            let bridge_len = (safe_z - from.z).max(0.0) + (safe_z - to.z).max(0.0) + {
                let (dx, dy) = (to.x - from.x, to.y - from.y);
                (dx * dx + dy * dy).sqrt()
            };

            if bridge_len >= air_len {
                for k in run_start..run_end {
                    if let Some(f) = air_flags.get_mut(k) {
                        *f = false;
                    }
                }
            }
        }
    }

    // Phase 2: emit the filtered toolpath. Track per-old-move where it landed
    // (or `None` if dropped) and which inserted moves are bridges.
    let mut result = Toolpath::new();
    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(moves.len());
    let mut bridge_ranges: Vec<std::ops::Range<usize>> = Vec::new();
    let mut in_air_run = false;

    for (i, m) in moves.iter().enumerate() {
        if air_flags[i] {
            // This cutting move is in air.
            if !in_air_run {
                let prev_target = if let Some(last) = result.moves.last() {
                    last.target
                } else if i > 0 {
                    moves[i - 1].target
                } else {
                    m.target
                };
                if prev_target.z < safe_z - 0.001 {
                    let b_start = result.moves.len();
                    result.rapid_to(P3::new(prev_target.x, prev_target.y, safe_z));
                    let b_end = result.moves.len();
                    bridge_ranges.push(b_start..b_end);
                }
                in_air_run = true;
            }
            // Drop this move from the output.
            old_to_new.push(None);
        } else {
            // This move is NOT in air (or is a rapid).
            if in_air_run {
                if m.move_type != MoveType::Rapid {
                    let source = if i > 0 { moves[i - 1].target } else { m.target };
                    let b_start = result.moves.len();
                    result.rapid_to(P3::new(source.x, source.y, safe_z));
                    if source.z < safe_z - 0.001 {
                        result.rapid_to(source);
                    }
                    let b_end = result.moves.len();
                    if b_end > b_start {
                        bridge_ranges.push(b_start..b_end);
                    }
                }
                in_air_run = false;
            }
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new.push(Some(new_idx..new_idx + 1));
        }
    }

    let new_n_moves = result.moves.len();
    let remap = MoveRemap { old_to_new };
    let new_spans = if spans_valid {
        let mut remapped = remap.remap_spans(&spans, new_n_moves);
        for r in bridge_ranges {
            remapped.push(Span::new(r.start, r.end, SpanKind::LinkBridge));
        }
        remapped
    } else {
        spans
    };

    Transformed::from_remap(
        AnnotatedToolpath {
            toolpath: result,
            spans: new_spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        },
        remap,
    )
}

#[cfg(test)]
mod tests;
