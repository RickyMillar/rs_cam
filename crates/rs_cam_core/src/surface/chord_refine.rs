//! Chord refinement against the drop-cutter surface: split a straight fed
//! move whose line does not track the drop-cutter surface within a
//! tolerance, at the worst probe, until every piece does.
//!
//! Moved out of `finish/scallop/ring_generation.rs` (G-TIERBURIAL,
//! `planning/tiered_finish_2026-09-30/RESULTS.md`) so that the scallop
//! rings and connectors, the unified-finish raster and waterline bands, and
//! the shared surface link (`finish/surface_link.rs`) use one
//! implementation. The algorithm and its constants are unchanged; the
//! rings use it as before (`ChordRefineCtx::from_cell`: both sides,
//! dropped split points, over-mesh coverage).

use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

/// Which probe result counts as "the surface is here".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChordCoverage {
    /// A finite drop-cutter contact AND the vertical ray at the probe XY
    /// passes through the mesh ([`point_is_covered`]). The scallop rings.
    OverMesh,
    /// A finite drop-cutter contact only. The surface link's own acceptance
    /// test (`build_surface_link`), kept as it was.
    Contact,
    /// A finite drop-cutter contact, and no contact reads as clear air (the
    /// probe is skipped, not a gap). A waterline contour: where the tool
    /// touches nothing at a probe, the level feed cannot cut there.
    ContactOrAir,
}

/// How a split point is placed on a chord that fails the tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChordInsert {
    /// At the worst probe's XY, dropped onto the drop-cutter surface. Every
    /// drop-cutter path: the rings, the raster, the link.
    Drop,
    /// At the chord's own Z, pushed sideways off the material until the
    /// drop-cutter floor there is at or below that Z
    /// ([`push_to_level`]). A waterline contour, whose feeds keep one Z: a
    /// dropped point on a near-vertical wall can sit millimetres above the
    /// level.
    Push,
}

/// Which deviation of a chord from the surface counts against the
/// tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChordSide {
    /// Above or below: the chord tracks the surface. The scallop rings,
    /// whose refinement is also a surface-fidelity pass.
    Both,
    /// Below only: the chord cuts into the surface (a gouge). A chord above
    /// the surface over a concave stretch leaves a cusp, not a gouge, and
    /// is kept. The raster and waterline bands and the surface link.
    Below,
}

/// Everything a chord refinement needs.
pub(crate) struct ChordRefineCtx<'a> {
    pub(crate) mesh: &'a TriangleMesh,
    pub(crate) index: &'a SpatialIndex,
    pub(crate) cutter: &'a dyn MillingCutter,
    pub(crate) stock_to_leave: f64,
    pub(crate) min_z: f64,
    /// Max allowed gap between a straight feed chord and the true
    /// drop-cutter surface under it (the op's path tolerance).
    pub(crate) chord_tolerance: f64,
    /// Spacing at which chords are probed against the surface. A chord is
    /// always probed at eight intervals or more (see [`refine_chord`]).
    pub(crate) probe_step: f64,
    pub(crate) coverage: ChordCoverage,
    pub(crate) side: ChordSide,
    pub(crate) insert: ChordInsert,
}

impl<'a> ChordRefineCtx<'a> {
    /// The context the scallop lifts rings with: the probe step comes from
    /// the generation grid's `cell_size`, `max(cell_size / 2,
    /// CHORD_REFINE_MIN_SEG_MM)`, and coverage is [`ChordCoverage::OverMesh`].
    pub(crate) fn from_cell(
        mesh: &'a TriangleMesh,
        index: &'a SpatialIndex,
        cutter: &'a dyn MillingCutter,
        stock_to_leave: f64,
        min_z: f64,
        chord_tolerance: f64,
        cell_size: f64,
    ) -> Self {
        Self {
            mesh,
            index,
            cutter,
            stock_to_leave,
            min_z,
            chord_tolerance,
            probe_step: (cell_size * 0.5).max(CHORD_REFINE_MIN_SEG_MM),
            coverage: ChordCoverage::OverMesh,
            side: ChordSide::Both,
            insert: ChordInsert::Drop,
        }
    }

    /// A gouge check for a fed path that is not a scallop ring: only a
    /// chord below the surface is split ([`ChordSide::Below`]), and the
    /// probe step is [`probe_step_for_tool`].
    #[allow(clippy::too_many_arguments)] // SAFETY: the ctx fields, named at the call site
    pub(crate) fn gouge_check(
        mesh: &'a TriangleMesh,
        index: &'a SpatialIndex,
        cutter: &'a dyn MillingCutter,
        stock_to_leave: f64,
        min_z: f64,
        chord_tolerance: f64,
        coverage: ChordCoverage,
    ) -> Self {
        Self {
            mesh,
            index,
            cutter,
            stock_to_leave,
            min_z,
            chord_tolerance,
            probe_step: probe_step_for_tool(cutter, chord_tolerance),
            coverage,
            side: ChordSide::Below,
            insert: ChordInsert::Drop,
        }
    }

    /// A gouge check for a constant-Z contour (a waterline): a chord below
    /// the drop-cutter surface is split at a point pushed sideways at the
    /// chord's Z ([`ChordInsert::Push`]); no contact is clear air
    /// ([`ChordCoverage::ContactOrAir`]).
    pub(crate) fn level_check(
        mesh: &'a TriangleMesh,
        index: &'a SpatialIndex,
        cutter: &'a dyn MillingCutter,
        stock_to_leave: f64,
        min_z: f64,
        chord_tolerance: f64,
    ) -> Self {
        Self {
            insert: ChordInsert::Push,
            ..Self::gouge_check(
                mesh,
                index,
                cutter,
                stock_to_leave,
                min_z,
                chord_tolerance,
                ChordCoverage::ContactOrAir,
            )
        }
    }
}

/// The probe step for a gouge check, from the tool and the tolerance: the
/// chord length whose sagitta over the tool's tip sphere (radius
/// `cusp_radius_mm`) equals the accept threshold `t =
/// CHORD_REFINE_ACCEPT_FRACTION x tolerance`, `sqrt(8 r t)` (sagitta `s =
/// L^2 / (8 r)` for `s` small against `r`). Where the drop-cutter surface
/// is the tip sphere itself (a convex edge), a chord this long between two
/// surface points stays within the threshold. Floored at
/// `CHORD_REFINE_MIN_SEG_MM`; a non-finite result (no tolerance) gives the
/// floor too. [`refine_chord`] probes a chord at eight intervals or more
/// whatever this step is.
pub(crate) fn probe_step_for_tool(cutter: &dyn MillingCutter, tolerance: f64) -> f64 {
    let r = cutter.cusp_radius_mm();
    let step = (8.0 * r * tolerance * CHORD_REFINE_ACCEPT_FRACTION).sqrt();
    if step.is_finite() {
        step.max(CHORD_REFINE_MIN_SEG_MM)
    } else {
        CHORD_REFINE_MIN_SEG_MM
    }
}

/// Whether `(x, y)` sits over real mesh surface, EXACTLY. See
/// `finish/scallop/ring_generation.rs` (`ring_to_3d`) for why the ring lift
/// needs the exact point-in-triangle test and not the generation grid's
/// coverage mask.
pub(crate) fn point_is_covered(ctx: &ChordRefineCtx<'_>, x: f64, y: f64) -> bool {
    crate::surface::dropcutter::point_is_over_mesh_xy(x, y, ctx.mesh, ctx.index)
}

/// The probe result at `(x, y)`: the surface Z (with `stock_to_leave`), or
/// `None` where the ctx's coverage says there is no surface.
fn surface_at(ctx: &ChordRefineCtx<'_>, x: f64, y: f64) -> Option<f64> {
    let cl = point_drop_cutter(x, y, ctx.mesh, ctx.index, ctx.cutter);
    let covered = match ctx.coverage {
        ChordCoverage::OverMesh => point_is_covered(ctx, x, y),
        ChordCoverage::Contact | ChordCoverage::ContactOrAir => true,
    };
    (cl.z.is_finite() && covered).then_some(cl.z + ctx.stock_to_leave)
}

/// The interior points of a straight feed from `a` to `b` (both exact
/// drop-cutter points), refined against the drop-cutter surface exactly as
/// a scallop ring chord is. `None` when the feed cannot track the surface
/// within the chord tolerance (a coverage gap, or a piece the depth cap
/// left failing): the caller then retracts, or keeps its own fallback.
///
/// G-TIERBURIAL (`planning/tiered_finish_2026-09-30/RESULTS.md`): the
/// scallop's ring-to-ring connector was one straight feed up to
/// `3 x cusp_r` long, never probed. On the rivmap100 fine tier it sat up to
/// 0.836 mm below the drop-cutter surface.
pub(crate) fn refine_track(a: P3, b: P3, ctx: &ChordRefineCtx<'_>) -> Option<Vec<P3>> {
    let mut out = Vec::new();
    let tracks = refine_chord(a, b, ctx, CHORD_REFINE_MAX_DEPTH, &mut out);
    (tracks && out.iter().all(|&(_, kept)| kept)).then(|| out.into_iter().map(|(p, _)| p).collect())
}

/// The interior points of a straight feed from `a` to `b`, refined as far
/// as the depth cap allows: every inserted point is a surface point
/// ([`ChordInsert::Drop`]) or a point at the chord's Z with the floor at or
/// below it ([`ChordInsert::Push`]), so no inserted point is under the
/// surface. `None` only on a coverage gap, where no surface point can be
/// inserted and the caller keeps its own fallback.
pub(crate) fn refine_best_effort(a: P3, b: P3, ctx: &ChordRefineCtx<'_>) -> Option<Vec<P3>> {
    let mut out = Vec::new();
    let _tracks = refine_chord(a, b, ctx, CHORD_REFINE_MAX_DEPTH, &mut out);
    out.iter()
        .all(|&(_, kept)| kept)
        .then(|| out.into_iter().map(|(p, _)| p).collect())
}

/// Floor (mm) on chord-refinement probe/segment spacing. Refinement must
/// not fragment paths into segments the junction/accel integrator pays
/// dearly for (P0 probe: sub-0.3 mm segment junctions dominate finishing
/// runtime) — 0.15 mm is half the wanaka raster reference pitch, i.e.
/// refined scallop is never more finely segmented than 2× the quality
/// reference it is chasing.
const CHORD_REFINE_MIN_SEG_MM: f64 = 0.15;

/// The shortest chord chord-refinement will SPLIT, and so half the shortest
/// segment it can create (mm).
///
/// Distinct from [`CHORD_REFINE_MIN_SEG_MM`], which floors how densely a
/// chord is *probed*: this floors what refinement is allowed to *emit*. The
/// two were the same number until M4 phase C, and conflating them is what
/// let a chord shorter than the probe step escape refinement entirely — the
/// mechanism behind the iso-field's −995 µm localised gouge and the shipped
/// cascade's −108.6 µm one (`CHECKPOINT_C_EVIDENCE.md` §3.8).
///
/// 50 µm: fifty times [`crate::toolpath::MIN_EMITTED_SEGMENT_MM`] (the
/// coarsest shipped post's coordinate quantum, PR-8d), and five times the
/// 10 µm junction-cost bar M4's segment-length gate is written in — so
/// refinement can never manufacture a segment either the post or the
/// accel integrator would object to. Refinement only ever splits a chord
/// that FAILS `chord_tolerance`, so on smooth ground this floor is never
/// reached and nothing is inserted at all.
const CHORD_REFINE_MIN_SPLIT_MM: f64 = 0.050;

/// Fraction of the op's chord tolerance at which refinement stops splitting.
///
/// Refinement measures a chord's deviation at a finite set of probes and
/// compares that to the tolerance — but the worst PROBE is not the worst
/// POINT, and on a convex feature the two differ by a third (measured, M4
/// phase C grooved block: worst probe 97.8 µm, true worst 131.7 µm at a
/// 100 µm tolerance). Accepting at `1.0` therefore emits chords that violate
/// the tolerance the operator set. The margin makes the sampling error
/// explicit rather than letting it show up as an over-cut.
const CHORD_REFINE_ACCEPT_FRACTION: f64 = 0.70;

/// Depth cap on recursive chord splitting. Combined with the probe-step
/// floor this bounds worst-case insertion on cliff edges, where the chord
/// error never converges and every level would otherwise split.
pub(crate) const CHORD_REFINE_MAX_DEPTH: usize = 5;

/// Probe the open interval between `a` and `b`; if the worst deviation
/// between chord and drop-cutter surface exceeds tolerance, insert the
/// exact surface point there and recurse into both halves. Pushes only
/// INTERIOR points (in order); the caller owns the endpoints. A coverage
/// gap under the chord (hole / mesh edge) pushes one excluded point so the
/// emission run-splitter retracts around it instead of feeding across.
///
/// Returns `true` when every piece of the chord was accepted (or is too
/// short to split), `false` when a coverage gap was found or the depth cap
/// stopped a piece that still failed. The ring lift ignores it;
/// [`refine_track`] refuses a feed on `false`.
pub(crate) fn refine_chord(
    a: P3,
    b: P3,
    ctx: &ChordRefineCtx<'_>,
    depth: usize,
    out: &mut Vec<(P3, bool)>,
) -> bool {
    if depth == 0 {
        return false;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() {
        return false;
    }
    if len <= 2.0 * CHORD_REFINE_MIN_SPLIT_MM {
        // Too short to split without emitting sub-floor segments, so probing
        // it could only ever discover an error refinement is not allowed to
        // correct. This is the ONLY length at which refinement declines.
        return true;
    }
    // At least one interior probe, ALWAYS.
    //
    // M4 phase C: this used to `return` when `ceil(len / probe_step) < 2`,
    // i.e. whenever a chord was shorter than the probe step — "nothing to
    // probe at this scale". That reasoning holds for a surface sampled on a
    // grid; it is false for a drop-cutter query, which is exact at any XY.
    // At a convex rim the tool-contact height is strongly convex over a
    // fraction of a cell, so a 0.27 mm chord can pass 0.7 mm under the
    // surface — and the old guard skipped it in silence.
    //
    // The exemption was invisible while every chord came from a decimated
    // offset ring (floored at `0.75 x cell`, always above `probe_step`).
    // Refinement's OWN halves are not: splitting a 0.56 mm chord yields two
    // 0.28 mm ones, which is how the shipped cascade reached a −108.6 µm
    // gouge and the undecimated iso-field reached −995 µm on the grooved
    // block (`CHECKPOINT_C_EVIDENCE.md` §3.8).
    //
    // And at least EIGHT intervals, so the check cannot alias past the worst
    // point. `probe_step` is sized from the generation grid (`cell / 2`),
    // which on a 0.75 mm cell affords a 0.7 mm chord exactly one interior
    // probe — at its midpoint. A chord crossing a groove wall has its worst
    // deviation nowhere near the middle: measured 131.7 µm at t = 0.296 on
    // the iso-field and 97.8 µm at t = 0.684 on the shipped cascade, both
    // invisible to a midpoint probe, the latter squeaking under a 100 µm
    // tolerance it was in fact violating. The grid sizes ring PLACEMENT; it
    // has no business sizing a tolerance check, which is an exact
    // drop-cutter query at any XY.
    //
    // M4 phase C set that minimum at FOUR and called it enough to stop the
    // aliasing. Wave 14 falsified that on the mixed-slope ribbon: a 0.374 mm
    // chord probed at t = 0.25/0.50/0.75 read under the 70 µm accept
    // threshold while its true worst sat at t = 0.316, 142.4 µm under the
    // surface — a probe-to-truth ratio of over 2.0 against the 1.35 the
    // accept margin was calibrated for. Four intervals did not survive a
    // change of endpoint PHASE, which means it was never bounding anything;
    // it was passing by luck of where the ring vertices happened to land.
    // Sampling error on a smooth surface falls with the square of the probe
    // spacing, so eight intervals buys back a factor of four — enough that
    // the accept margin is doing the job it is documented to do rather than
    // covering for the probe set. It costs one extra drop-cutter query per
    // three on chords that are probed at all.
    let step = ctx
        .probe_step
        .min(len * 0.125)
        .max(CHORD_REFINE_MIN_SPLIT_MM);
    let segments = (len / step).ceil().max(2.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let segments = segments as usize;

    // (t, surface_z, |err|) of the worst interior probe.
    let mut worst: Option<(f64, f64, f64)> = None;
    for i in 1..segments {
        let t = i as f64 / segments as f64;
        let x = a.x + dx * t;
        let y = a.y + dy * t;
        let Some(surface_z) = surface_at(ctx, x, y) else {
            if ctx.coverage == ChordCoverage::ContactOrAir {
                continue; // the tool touches nothing here
            }
            out.push((P3::new(x, y, ctx.min_z + ctx.stock_to_leave), false));
            return false;
        };
        let chord_z = a.z + (b.z - a.z) * t;
        let err = match ctx.side {
            ChordSide::Both => (surface_z - chord_z).abs(),
            ChordSide::Below => surface_z - chord_z,
        };
        if worst.is_none_or(|(_, _, we)| err > we) {
            worst = Some((t, surface_z, err));
        }
    }
    let Some((t, surface_z, err)) = worst else {
        return true;
    };
    // Accept with a margin, because `err` is the worst PROBE, not the worst
    // point: a finite probe set on a convex rim always understates, and
    // accepting at exactly the tolerance therefore ships chords that violate
    // it. Measured on the M4 grooved block at a 100 µm tolerance: worst probe
    // 97.8 µm, true worst 131.7 µm. The margin buys the difference back and
    // costs points only on chords that were already failing.
    if err <= ctx.chord_tolerance * CHORD_REFINE_ACCEPT_FRACTION {
        return true;
    }
    // The split point must not orphan a sub-floor segment on either side, so
    // it is CLAMPED into the admissible band rather than abandoned.
    //
    // Wave 14: this used to `return` whenever the worst probe sat within
    // `CHORD_REFINE_MIN_SPLIT_MM` of either end — i.e. a chord whose worst
    // deviation is near an endpoint was left *entirely* unrefined, tolerance
    // violation and all. Denser probing made that worse rather than better,
    // which is how it surfaced: on the narrow ridge a 0.363 mm chord whose
    // true worst sits at t = 0.158 read its worst probe at t = 0.125, whose
    // head (45 µm) is under the 50 µm floor, so refinement declined and
    // shipped a 216.9 µm violation of a 100 µm tolerance. With four probes
    // the same chord split at t = 0.25 and passed — the guard was rewarding
    // a coarser check, which is exactly backwards.
    //
    // Clamping keeps the floor's promise (no sub-50 µm segment is emitted)
    // while still cutting the chord in two, and the offending stretch lands
    // in the longer half where recursion can reach it. `len > 2 ×
    // MIN_SPLIT` is guaranteed above, so the band is never empty.
    let t_floor = CHORD_REFINE_MIN_SPLIT_MM / len;
    let t_split = t.clamp(t_floor, 1.0 - t_floor);
    let (wx, wy) = (a.x + dx * t_split, a.y + dy * t_split);
    let w = match ctx.insert {
        ChordInsert::Drop => {
            let split_z = if (t_split - t).abs() < f64::EPSILON {
                surface_z
            } else {
                // The clamp moved the point, so the surface height there is a
                // different query — never re-use the probe's answer for it.
                let Some(z) = surface_at(ctx, wx, wy) else {
                    out.push((P3::new(wx, wy, ctx.min_z + ctx.stock_to_leave), false));
                    return false;
                };
                z
            };
            P3::new(wx, wy, split_z)
        }
        ChordInsert::Push => {
            let level = a.z + (b.z - a.z) * t_split;
            let Some(w) = push_to_level(ctx, wx, wy, (dx / len, dy / len), level, 0.5 * len) else {
                return false;
            };
            w
        }
    };
    let head = refine_chord(a, w, ctx, depth - 1, out);
    out.push((w, true));
    let tail = refine_chord(w, b, ctx, depth - 1, out);
    head && tail
}

/// The point at Z `level` nearest `(x, y)` along the chord normal
/// (`(ux, uy)` is the chord's unit direction) where the drop-cutter floor is
/// at or below `level`: the tool there, at that Z, touches the surface or
/// nothing. The walk steps out on both sides at once by the split floor
/// (`CHORD_REFINE_MIN_SPLIT_MM`), up to `reach`, and bisects on the first
/// side that clears. `None` when neither side clears within `reach`.
///
/// The caller passes half the chord's length as `reach`: the contour runs
/// through both chord ends, so a contour point further from the chord than
/// that belongs to another stretch of the contour, not to this chord.
fn push_to_level(
    ctx: &ChordRefineCtx<'_>,
    x: f64,
    y: f64,
    (ux, uy): (f64, f64),
    level: f64,
    reach: f64,
) -> Option<P3> {
    let at = |s: f64| (x - uy * s, y + ux * s);
    let clear = |s: f64| {
        let (px, py) = at(s);
        surface_at(ctx, px, py).is_none_or(|floor| floor <= level)
    };
    let h = CHORD_REFINE_MIN_SPLIT_MM;
    let mut lo = 0.0;
    let (sign, mut hi) = loop {
        let s = (lo + h).min(reach);
        if clear(s) {
            break (1.0, s);
        }
        if clear(-s) {
            break (-1.0, s);
        }
        if s >= reach {
            return None;
        }
        lo = s;
    };
    // 24 halvings of a step of at most `h`: well under a micrometre.
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if clear(sign * mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let (px, py) = at(sign * hi);
    Some(P3::new(px, py, level))
}

/// What [`refine_fed_cut_chords`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FedChordRefinement {
    /// Fed `FinishingCut` chords that got at least one inserted point.
    pub(crate) chords_split: usize,
    /// Surface points inserted in total.
    pub(crate) points_inserted: usize,
    /// Chords left straight because a probe found no surface under them
    /// ([`refine_best_effort`] returned `None`).
    pub(crate) coverage_gaps: usize,
}

/// Refine every fed `FinishingCut` linear move of `tp` against the
/// drop-cutter surface ([`refine_best_effort`]): the inserted points are fed
/// at the move's own feed rate and intent, before the move. Rapids, arcs and
/// moves of any other intent are copied as they are, so a vertical plunge
/// or retract is never touched. Use it on a path whose move indices nobody
/// holds yet (no annotations, no spans).
///
/// G-TIERBURIAL: `raster_toolpath_from_grid` feeds straight between lattice
/// points and across row turnarounds, and a waterline contour feeds
/// straight between fiber crossings, with no chord check. On the rivmap100
/// fine tier those chords went 0.325 mm (raster) and 0.158 mm (waterline)
/// below the drop-cutter surface.
pub(crate) fn refine_fed_cut_chords(
    tp: Toolpath,
    ctx: &ChordRefineCtx<'_>,
) -> (Toolpath, FedChordRefinement) {
    let mut stats = FedChordRefinement::default();
    let mut out = Toolpath::new();
    out.moves.reserve(tp.moves.len());
    let mut prev: Option<P3> = None;
    for m in tp.moves {
        if let (Some(a), MoveType::Linear { feed_rate }, MoveIntent::FinishingCut) =
            (prev, m.move_type, m.intent)
        {
            match refine_best_effort(a, m.target, ctx) {
                Some(pts) if !pts.is_empty() => {
                    stats.chords_split += 1;
                    stats.points_inserted += pts.len();
                    for p in pts {
                        out.feed_to_with_intent(p, feed_rate, MoveIntent::FinishingCut);
                    }
                }
                Some(_) => {}
                None => stats.coverage_gaps += 1,
            }
        }
        prev = Some(m.target);
        out.moves.push(m);
    }
    (out, stats)
}
