//! Contour-parallel pocket clearing operation.
//!
//! Takes a 2D polygon boundary and generates a toolpath that clears material
//! using concentric inward offsets. Tool radius compensation is applied
//! automatically so the tool edge follows the pocket wall.

use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{FlattenPolicy, OffsetRingSet, Polygon2};
use crate::toolpath::{MoveIntent, Toolpath};

/// Parameters for pocket clearing.
pub struct PocketParams {
    /// Tool radius in mm (half of tool diameter).
    pub tool_radius: f64,
    /// Distance between concentric passes in mm.
    pub stepover: f64,
    /// Z height of the cut in mm (typically negative, e.g. -3.0 for 3mm deep).
    pub cut_depth: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Climb milling: true = CW tool direction (climb), false = CCW (conventional).
    pub climb: bool,
}

/// Generate a contour-parallel pocket clearing toolpath.
///
/// Process:
/// 1. Offset boundary inward by tool radius (tool compensation)
/// 2. Generate concentric inward offsets by stepover
/// 3. Convert contours to toolpath with rapids, plunges, and contour following
///
/// Contours are cut from outermost to innermost (finish pass first).
/// Returns an empty toolpath if the pocket is too small for the tool.
#[tracing::instrument(skip(polygon, params), fields(
    tool_radius = params.tool_radius,
    stepover = params.stepover,
))]
pub fn pocket_toolpath(polygon: &Polygon2, params: &PocketParams) -> Toolpath {
    let never_cancel = || false;
    // infallible: cancel closure always returns false, so Cancelled is unreachable
    #[allow(clippy::expect_used)]
    pocket_toolpath_with_cancel(polygon, params, &never_cancel)
        .expect("non-cancellable pocket toolpath should never be cancelled")
}

/// Cancellable variant of [`pocket_toolpath`]. Polls `cancel` once per
/// concentric offset ring via [`pocket_contours_with_cancel`] — pocket's
/// `loop {}` (planning/finishing_stack_review_2026-07.md S.5: "worst
/// loops... pocket (unbounded offset loop)") is otherwise unbounded for a
/// large polygon with a tiny stepover.
pub fn pocket_toolpath_with_cancel(
    polygon: &Polygon2,
    params: &PocketParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    Ok(pocket_toolpath_reported_with_cancel(polygon, params, cancel)?.0)
}

/// [`pocket_toolpath_with_cancel`] with Checkpoint C's offset failure
/// channel attached.
///
/// The second element counts offset calls that FAILED rather than collapsed
/// — the compensation offset plus every ring of the cascade. It matters most
/// here, and F-12 is why: the cascade's only exit is a collapsed ring, and a
/// contained panic IS a collapsed ring to that loop. An inlay's female pocket
/// stopped early on a `debug_assert!` inside a transitive dependency, left
/// material standing, and reported a successful generate — the loop cannot
/// tell "the pocket is finished" from "the offset broke".
pub fn pocket_toolpath_reported_with_cancel(
    polygon: &Polygon2,
    params: &PocketParams,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, PocketCascadeReport), Cancelled> {
    let (contours, report) =
        pocket_contours_reported_with_cancel(polygon, params.tool_radius, params.stepover, cancel)?;
    Ok((contours_to_toolpath(&contours, params), report))
}

/// Generate the 2D contour rings for pocket clearing (no Z, no toolpath yet).
///
/// Useful for visualization or when you need the geometry separately.
pub fn pocket_contours(polygon: &Polygon2, tool_radius: f64, stepover: f64) -> Vec<Vec<P2>> {
    let never_cancel = || false;
    // infallible: cancel closure always returns false, so Cancelled is unreachable
    #[allow(clippy::expect_used)]
    pocket_contours_with_cancel(polygon, tool_radius, stepover, &never_cancel)
        .expect("non-cancellable pocket contours should never be cancelled")
}

/// Checkpoint C, Q3 (F-10): what stopped a pocket ring cascade, when it was
/// not the cascade running out of geometry.
///
/// The loop's only exit used to be `rings.is_empty()`. That is fine while
/// every offset shrinks, and whether it shrinks depends on the input's
/// WINDING, because cavalier's offset sign is relative to the polyline's own
/// direction: on a CW exterior a positive distance GROWS the ring. W4
/// measured the consequence — a CW-wound fixture grew 13x per ring and
/// allocated **22.9 GB without terminating**, with nothing in the log naming
/// the cell. `pocket.rs`'s own comment already said the quiet part: *"Only
/// the cancel hook made that a hang instead of a lock-up, which is not the
/// same thing as being bounded."*
///
/// Reachability is LOW and asserted rather than assumed — both importers call
/// `Polygon2::ensure_winding`, as does `detect_containment` — but the bound
/// was missing from the LOOP, and the winding guard is a property of two
/// importers, not of the cascade. Any future producer that does not normalise
/// re-arms it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeBound {
    /// The ring count passed the geometric maximum.
    ///
    /// This bound cannot fire on a convergent cascade: each ring erodes at
    /// least `stepover` from every side, so a shape whose largest extent is
    /// `E` cannot need more than `E / stepover` rings before it collapses.
    /// Exceeding that means the cascade is NOT converging — a
    /// contract-violating CW exterior, a non-finite coordinate, or a
    /// non-positive stepover.
    RingCount,
    /// The wall-clock budget ran out.
    ///
    /// The safety net for the other failure shape: not too many rings, but
    /// rings that each take too long. Machine-dependent by nature, so it is
    /// set generously and is the bound of last resort.
    WallClock,
}

/// What a pocket ring cascade did, beyond the contours it returned.
///
/// The established pattern (`scallop::ScallopReport`): the algorithm reports,
/// the adapter records into [`crate::compute::execute::GenerationFindings`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PocketCascadeReport {
    /// Offset calls that FAILED rather than collapsed — Checkpoint C's Q1
    /// channel, counted over the compensation offset and every ring.
    pub offset_failures: usize,
    /// Rings the cascade emitted.
    pub rings: usize,
    /// Which bound stopped it, if a bound did. `None` is the normal case:
    /// the cascade collapsed on its own.
    pub stopped_by: Option<CascadeBound>,
    /// XY-projected area (mm²) of ring interior still standing when a bound
    /// stopped the cascade — material this pocket will NOT clear.
    ///
    /// `None` when no bound fired, which is the same three-valued contract
    /// [`crate::compute::config::ToolpathStats::truncated_core_mm2`] carries
    /// and is why it can be recorded straight onto that existing channel
    /// rather than needing a new one. On the divergent-winding case this
    /// number is enormous, which is exactly the right signal.
    pub truncated_core_mm2: Option<f64>,
}

/// Wall-clock budget for one pocket ring cascade.
///
/// Deliberately generous: this is the bound of last resort, and a tight
/// wall-clock in a debug build would fire for reasons that have nothing to do
/// with divergence. The `pocket_cascade_terminates_on_the_reflex_cross`
/// sentry — the hardest shape this cascade is known to meet — completes in
/// well under a second.
const CASCADE_WALL_CLOCK: std::time::Duration = std::time::Duration::from_secs(120);

/// The geometric maximum number of rings a CONVERGENT cascade can need.
///
/// Each ring erodes at least `stepover` from every side, so a shape whose
/// largest extent is `E` collapses within `E / stepover` rings. The `+ 2`
/// absorbs the arc-join and flattening slack; the floor of 8 keeps a
/// degenerate-but-finite input from being capped to nothing.
///
/// A non-finite or non-positive stepover — which would otherwise offset by
/// zero forever, never emptying and never erroring — falls to the floor and
/// terminates.
fn geometric_ring_cap(polygon: &Polygon2, stepover: f64) -> usize {
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &polygon.exterior {
        if !p.x.is_finite() || !p.y.is_finite() {
            continue;
        }
        min = (min.0.min(p.x), min.1.min(p.y));
        max = (max.0.max(p.x), max.1.max(p.y));
    }
    let extent = (max.0 - min.0).max(max.1 - min.1);
    if !extent.is_finite() || extent <= 0.0 || !stepover.is_finite() || stepover <= 0.0 {
        return 8;
    }
    let rings = (extent / stepover).ceil();
    if !rings.is_finite() || rings > 1e7 {
        return 10_000_000;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rings = rings as usize;
    rings.saturating_add(2).max(8)
}

/// Cancellable variant of [`pocket_contours`]. Checks `cancel` as its very
/// first statement, then once per stepover ring (the outer `loop {}`
/// below) — the loop terminates only when an offset ring collapses to
/// nothing, so a large simple polygon with a very small stepover can
/// otherwise run for a long time with no way to interrupt it.
pub fn pocket_contours_with_cancel(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<P2>>, Cancelled> {
    Ok(pocket_contours_reported_with_cancel(polygon, tool_radius, stepover, cancel)?.0)
}

/// [`pocket_contours_with_cancel`] with Checkpoint C's offset failure
/// channel attached — see [`pocket_toolpath_reported_with_cancel`] for why
/// this cascade is the one that most needs it.
pub fn pocket_contours_reported_with_cancel(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<Vec<P2>>, PocketCascadeReport), Cancelled> {
    check_cancel(cancel)?;
    // First offset: tool radius compensation (tool edge touches wall)
    let (compensated, compensation_failure) =
        crate::polygon::offset_polygon_reported(polygon, tool_radius);
    let mut report = PocketCascadeReport {
        offset_failures: usize::from(compensation_failure.is_some()),
        rings: 0,
        stopped_by: None,
        truncated_core_mm2: None,
    };

    // Checkpoint C, Q3 (F-10): the two explicit bounds. Both are derived or
    // generous rather than tuned — see `geometric_ring_cap` and
    // `CASCADE_WALL_CLOCK` — because the job here is to make divergence
    // terminate and be reported, not to cap legitimate work.
    let ring_cap = geometric_ring_cap(polygon, stepover);
    let started = std::time::Instant::now();

    let mut all_contours: Vec<Vec<P2>> = Vec::new();

    for comp in &compensated {
        if comp.exterior.len() < 3 {
            continue;
        }
        // Compensated boundary is the outermost cutting contour
        all_contours.push(comp.exterior.clone());

        // Include hole contours (reversed to CCW so direction logic works uniformly)
        for hole in &comp.holes {
            if hole.len() >= 3 {
                let mut reversed = hole.clone();
                reversed.reverse(); // CW→CCW
                all_contours.push(reversed);
            }
        }

        // Generate inner contours by repeated stepover offset.
        //
        // **Checkpoint D (2026-08-03): this cascade carries ARCS.** It used to
        // feed each flattened offset straight back into `offset_polygon`, and
        // that is the vertex-doubling defect: `Polygon2::from_pline` discards
        // every arc join's bulge and keeps both of its endpoints, so one
        // reflex corner becomes two shallower reflex corners, each of which
        // arc-joins on the next pass. Added vertices == arc-join segments,
        // 1:1, measured on eight fixtures.
        //
        // Pocket was the consumer that paid the most for it and the one
        // nobody had bounded. On a **twelve-vertex cross at a 0.5 mm
        // stepover** this function DID NOT FINISH IN 20 SECONDS, and
        // `polygon::pocket_offsets` — the same loop without the cancel hook,
        // since retired — ran **13 minutes to 386 MB** before being killed
        // (`CHECKPOINT_D_EVIDENCE.md` §8). Only the cancel hook made that a
        // hang instead of a lock-up, which is not the same thing as being
        // bounded.
        //
        // Keeping the arcs removes the mechanism: the cascade's vertex count
        // now SHRINKS as the boundary erodes. It also fixes a fidelity defect
        // that was never named as one — each discarded bulge cut its corner
        // by the join's sagitta, `r·(1 − cos(θ/2))`, i.e. **29% of the offset
        // distance at a 90° join**, unbounded and compounding inward.
        //
        // The flatten is DEVIATION-ONLY (`untoleranced`, 10 µm) with no
        // sampling bound, and the contrast with scallop is the point:
        // scallop reads ring vertices as drop-cutter SAMPLE POSITIONS and so
        // must state a spacing (`scallop::RingSampleBound`), while a pocket
        // ring IS the cut — a straight run across a pocket floor is a
        // straight cut, and subdividing it would buy nothing. 10 µm is 14×
        // tighter than the corner cut this path shipped before today.
        let flatten = FlattenPolicy::untoleranced();
        let mut rings = OffsetRingSet::from_polygon(comp);
        loop {
            check_cancel(cancel)?;
            // The bounds are checked BEFORE the offset, so the ring that
            // would have exceeded them is never allocated. On the divergent
            // case that is the difference between stopping and adding
            // another 13x of geometry.
            let bound = if report.rings >= ring_cap {
                Some(CascadeBound::RingCount)
            } else if started.elapsed() >= CASCADE_WALL_CLOCK {
                Some(CascadeBound::WallClock)
            } else {
                None
            };
            if let Some(bound) = bound {
                // What the cascade would still have had to clear. Recorded
                // rather than discarded: a truncated pocket leaves material,
                // and the whole point of a bound is that it must not be the
                // silent kind.
                let standing: f64 = rings
                    .to_polygons(flatten)
                    .iter()
                    .map(|p| p.area().abs())
                    .sum();
                report.stopped_by = Some(bound);
                report.truncated_core_mm2 = Some(standing);
                tracing::warn!(
                    ?bound,
                    rings = report.rings,
                    ring_cap,
                    stepover,
                    standing_mm2 = standing,
                    elapsed_s = started.elapsed().as_secs_f64(),
                    "pocket ring cascade hit an explicit bound and stopped — \
                     it is NOT converging. The usual cause is a CW-wound \
                     exterior (Polygon2's contract is CCW; cavalier's offset \
                     sign is relative to the polyline's own direction, so a \
                     positive distance GROWS a CW ring), a non-finite \
                     coordinate, or a non-positive stepover. Material is left \
                     standing."
                );
                break;
            }
            let (next, failure) = rings.offset_reported(stepover);
            rings = next;
            if failure.is_some() {
                report.offset_failures += 1;
            }
            if rings.is_empty() {
                break;
            }
            report.rings += 1;
            let mut any = false;
            for inner in rings.to_polygons(flatten) {
                if inner.exterior.len() < 3 {
                    continue;
                }
                any = true;
                all_contours.push(inner.exterior.clone());

                // Include hole contours from inner offsets
                for hole in &inner.holes {
                    if hole.len() >= 3 {
                        let mut reversed = hole.clone();
                        reversed.reverse(); // CW→CCW
                        all_contours.push(reversed);
                    }
                }
            }
            if !any {
                break;
            }
        }
    }

    Ok((all_contours, report))
}

/// Convert 2D contour rings into a 3D toolpath at the given parameters.
///
/// S.7 (planning/finishing_stack_review_2026-07.md): each contour is emitted
/// via the shared `emit_closed_contour_with_intent` rapid→plunge→feed→close→
/// retract envelope instead of open-coding it — byte-identical to the
/// previous hand-rolled sequence for every contour `pocket_contours`
/// produces (all are pre-filtered to >= 3 points before being pushed).
fn contours_to_toolpath(contours: &[Vec<P2>], params: &PocketParams) -> Toolpath {
    let mut tp = Toolpath::new();

    for contour in contours {
        if contour.is_empty() {
            continue;
        }

        // Optionally reverse for climb milling (CW direction)
        let ordered: Vec<&P2> = if params.climb {
            contour.iter().rev().collect()
        } else {
            contour.iter().collect()
        };
        let points: Vec<P3> = ordered
            .iter()
            .map(|p| P3::new(p.x, p.y, params.cut_depth))
            .collect();

        tp.emit_closed_contour_with_intent(
            &points,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
            MoveIntent::ClearingCut,
        );
    }

    tp
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn default_params() -> PocketParams {
        PocketParams {
            tool_radius: 3.175, // 1/4" endmill
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            climb: false,
        }
    }

    /// **The §8 defect, as a sentry.** A twelve-vertex cross at a 0.5 mm
    /// stepover is not an exotic input — it is the shape class 2.5D pocketing
    /// actually meets — and before Checkpoint D this call **did not finish in
    /// 20 seconds** (and its no-cancel twin ran 13 minutes to 386 MB).
    ///
    /// The bound asserted here is deliberately generous: the point is
    /// terminates-at-all versus exponential, not a performance pin, and a
    /// tight wall-clock in a debug-build test would be flaky for reasons that
    /// have nothing to do with this cascade.
    ///
    /// The ring-count check is the other half. Terminating early and quietly
    /// would also "pass" a timing test — cavalier can panic on the `Shape`
    /// path and `offset_polygon` maps that to a collapsed offset — so the
    /// erosion depth is checked too. A 20 mm-wide arm eroded 0.5 mm per side
    /// per ring admits ~20 rings before it closes.
    #[test]
    fn pocket_cascade_terminates_on_the_reflex_cross() {
        let (a, b) = (20.0, 60.0);
        let cross = Polygon2::new(vec![
            P2::new(a, 0.0),
            P2::new(b, 0.0),
            P2::new(b, a),
            P2::new(b + a, a),
            P2::new(b + a, b),
            P2::new(b, b),
            P2::new(b, b + a),
            P2::new(a, b + a),
            P2::new(a, b),
            P2::new(0.0, b),
            P2::new(0.0, a),
            P2::new(a, a),
        ]);

        let start = std::time::Instant::now();
        let contours = pocket_contours(&cross, 0.0, 0.5);
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(20),
            "the pocket cascade must terminate: took {elapsed:?} on a \
             12-vertex cross at a 0.5 mm stepover (pre-Checkpoint-D this \
             call did not return in 20 s)"
        );
        assert!(
            contours.len() >= 15,
            "expected the cascade to erode a 20 mm arm over many rings, got \
             {} contours — a cascade that collapses early passes a timing \
             check for the wrong reason",
            contours.len()
        );
    }

    /// **F-10, bounded** — Checkpoint C, Q3.
    ///
    /// W4's `the_pocket_ring_cascade_is_bounded_only_by_collapse` proved the
    /// parent unbounded, and it did so *safely*: it drives
    /// `OffsetRingSet::offset` directly under its own 40-ring cap and
    /// measures the AREA TREND, showing a CW-wound square growing past 4x
    /// with no sign of collapsing. That probe still passes and must — the
    /// cascade PRIMITIVE is deliberately still unbounded, because a bound is
    /// a consumer's policy, not a geometry type's.
    ///
    /// This is the other half: the same fixture through the PRODUCTION
    /// entry point, which now terminates.
    ///
    /// # Why this fails on the unbounded parent within a small time budget
    ///
    /// The cancel closure trips after `BUDGET`. Pre-fix, the only exit from
    /// the loop was `rings.is_empty()`, which a CW exterior never reaches, so
    /// the call ran until the closure fired and came back `Err(Cancelled)` —
    /// this test's first assertion. Post-fix the ring cap stops it far
    /// sooner and the call returns `Ok`. The budget also means the parent is
    /// never given long enough to reach the 22.9 GB W4 measured; the growth
    /// itself is evidenced by W4's probe rather than re-run here.
    ///
    /// The smallest fixture that demonstrates growth is W4's: a 60 mm square,
    /// CW-wound, at a 2.4 mm stepover.
    #[test]
    fn the_pocket_cascade_is_bounded_on_a_diverging_cw_exterior() {
        use std::time::{Duration, Instant};

        const BUDGET: Duration = Duration::from_secs(5);
        const STEP: f64 = 2.4;
        const SIZE: f64 = 60.0;

        // `Polygon2`'s contract is CCW (polygon.rs:9-12) and nothing
        // validates it. Reversing the exterior is all it takes.
        let mut cw = Polygon2::rectangle(0.0, 0.0, SIZE, SIZE);
        cw.exterior.reverse();
        assert!(
            !cw.has_correct_winding(),
            "this fixture must actually be CW, or it proves nothing"
        );

        let started = Instant::now();
        let deadline = || started.elapsed() > BUDGET;
        let (contours, report) = pocket_contours_reported_with_cancel(&cw, 0.0, STEP, &deadline)
            .unwrap_or_else(|_| {
                panic!(
                    "the cascade did not terminate within {BUDGET:?} on a \
                     CW-wound {SIZE} mm square — it is bounded only by \
                     collapse, which a growing ring never reaches. This is \
                     F-10, and it allocated 22.9 GB without terminating when \
                     W4 met it."
                )
            });

        assert_eq!(
            report.stopped_by,
            Some(CascadeBound::RingCount),
            "a diverging cascade must stop on the RING CAP, not on the \
             wall-clock net — if this reports WallClock the cap is not doing \
             the work and the bound is machine-dependent"
        );
        let standing = report
            .truncated_core_mm2
            .expect("a bound that fires must report the material it left");
        assert!(
            standing > 0.0,
            "a diverging cascade leaves ring interior standing: {standing}"
        );
        assert!(
            !contours.is_empty(),
            "the bound truncates the cascade; it does not delete the work \
             already done"
        );
        println!(
            "CW {SIZE} mm square @ {STEP} mm: stopped by {:?} after {} rings, \
             {standing:.0} mm² standing, {:?} elapsed",
            report.stopped_by,
            report.rings,
            started.elapsed(),
        );
    }

    /// The other side of the bound: it must not fire on convergent work.
    ///
    /// A cap that also truncates legitimate pockets would be a worse defect
    /// than the one it fixes, and it would be invisible — the cascade would
    /// simply stop early and the pocket would look finished.
    #[test]
    fn the_cascade_bound_never_fires_on_convergent_work() {
        // The reflex cross, the hardest shape this cascade is known to meet.
        let (a, b) = (20.0, 60.0);
        let cross = Polygon2::new(vec![
            P2::new(a, 0.0),
            P2::new(b, 0.0),
            P2::new(b, a),
            P2::new(b + a, a),
            P2::new(b + a, b),
            P2::new(b, b),
            P2::new(b, b + a),
            P2::new(a, b + a),
            P2::new(a, b),
            P2::new(0.0, b),
            P2::new(0.0, a),
            P2::new(a, a),
        ]);
        let never = || false;
        for (label, poly, radius, step) in [
            ("reflex cross @ 0.5", cross, 0.0, 0.5),
            (
                "600 mm plate @ 0.05",
                Polygon2::rectangle(0.0, 0.0, 600.0, 600.0),
                0.0,
                0.05,
            ),
            (
                "30 mm square, Ø6.35 tool",
                Polygon2::rectangle(0.0, 0.0, 30.0, 30.0),
                3.175,
                2.0,
            ),
        ] {
            let (_, report) =
                pocket_contours_reported_with_cancel(&poly, radius, step, &never).unwrap();
            assert_eq!(
                report.stopped_by, None,
                "{label}: a convergent cascade must collapse on its own — a \
                 bound firing here means the cap is too tight and pockets \
                 are being silently truncated"
            );
            assert_eq!(
                report.truncated_core_mm2, None,
                "{label}: no bound fired, so nothing was left standing to \
                 measure — `Some(0.0)` here would be a fabricated \
                 measurement (X-19)"
            );
        }
    }

    /// A zero or negative stepover offsets by zero (or outward) forever: the
    /// rings never empty and the loop never exits. The geometric cap's floor
    /// is what makes that terminate.
    ///
    /// `NaN` is checked separately and deliberately NOT asserted to hit the
    /// cap. It terminates for a different reason — the offset panics inside
    /// `static_aabb2d_index`'s bounding-box check, the Q1 chokepoint contains
    /// it, and a contained failure reads as a collapsed ring. That is a
    /// debug-build path (`debug_assert!`; Checkpoint C, Q4), so the only
    /// property worth asserting across builds is that the call comes back at
    /// all. The stderr panic line it prints is the containment working.
    #[test]
    fn a_non_positive_stepover_terminates() {
        let sq = Polygon2::rectangle(0.0, 0.0, 60.0, 60.0);
        let never = || false;
        for step in [0.0, -1.0] {
            let (_, report) = pocket_contours_reported_with_cancel(&sq, 0.0, step, &never)
                .unwrap_or_else(|_| panic!("stepover {step} must terminate, not hang"));
            assert_eq!(
                report.stopped_by,
                Some(CascadeBound::RingCount),
                "stepover {step}: the ring cap is the only thing that can \
                 stop an offset that never shrinks"
            );
        }
        let (_, nan_report) = pocket_contours_reported_with_cancel(&sq, 0.0, f64::NAN, &never)
            .expect("a NaN stepover must terminate, not hang");
        println!("NaN stepover terminated via {:?}", nan_report.stopped_by);
    }

    #[test]
    fn test_pocket_contours_square() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let contours = pocket_contours(&sq, 3.175, 2.0);

        // 30mm square, tool radius 3.175mm → compensated is ~23.65mm
        // Then stepover 2.0mm → about 5-6 inner contours
        assert!(
            contours.len() >= 3,
            "Expected at least 3 contours, got {}",
            contours.len()
        );

        // Each contour should be a closed ring (at least 3 points)
        for (i, contour) in contours.iter().enumerate() {
            assert!(
                contour.len() >= 3,
                "Contour {} has only {} points",
                i,
                contour.len()
            );
        }
    }

    #[test]
    fn test_pocket_toolpath_basic() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = pocket_toolpath(&sq, &params);

        assert!(!tp.moves.is_empty(), "Pocket toolpath should have moves");

        // All cutting moves should be at cut_depth
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && feed_rate == params.feed_rate
            {
                assert!(
                    (m.target.z - params.cut_depth).abs() < 1e-10,
                    "Cutting move at z={} should be at cut_depth={}",
                    m.target.z,
                    params.cut_depth
                );
            }
        }

        // All rapid moves should be at safe_z
        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                assert!(
                    (m.target.z - params.safe_z).abs() < 1e-10,
                    "Rapid at z={} should be at safe_z={}",
                    m.target.z,
                    params.safe_z
                );
            }
        }
    }

    #[test]
    fn test_pocket_toolpath_structure() {
        // Verify the rapid-plunge-cut-retract pattern
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = pocket_toolpath(&sq, &params);

        let contours = pocket_contours(&sq, params.tool_radius, params.stepover);
        let n_contours = contours.len();

        // Count rapid moves: 2 per contour (approach + retract)
        let n_rapids = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        assert_eq!(
            n_rapids,
            n_contours * 2,
            "Expected 2 rapids per contour ({} contours), got {}",
            n_contours,
            n_rapids
        );

        // Count plunge moves (feed at plunge_rate)
        let n_plunges = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.plunge_rate).abs() < 1e-10))
            .count();
        assert_eq!(
            n_plunges, n_contours,
            "Expected 1 plunge per contour ({} contours), got {}",
            n_contours, n_plunges
        );
    }

    #[test]
    fn test_pocket_too_small_for_tool() {
        // 5mm square with 3.175mm radius tool → pocket collapses
        let tiny = Polygon2::rectangle(0.0, 0.0, 5.0, 5.0);
        let params = default_params();
        let tp = pocket_toolpath(&tiny, &params);

        assert!(
            tp.moves.is_empty(),
            "Pocket too small for tool should produce empty toolpath, got {} moves",
            tp.moves.len()
        );
    }

    #[test]
    fn test_pocket_climb_vs_conventional() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);

        let mut conv_params = default_params();
        conv_params.climb = false;
        let conv_tp = pocket_toolpath(&sq, &conv_params);

        let mut climb_params = default_params();
        climb_params.climb = true;
        let climb_tp = pocket_toolpath(&sq, &climb_params);

        // Both should have the same number of moves
        assert_eq!(conv_tp.moves.len(), climb_tp.moves.len());

        // Both should have the same total cutting distance (same contours, different direction)
        let conv_dist = conv_tp.total_cutting_distance();
        let climb_dist = climb_tp.total_cutting_distance();
        assert!(
            (conv_dist - climb_dist).abs() < 1.0,
            "Cutting distance should be similar: conv={}, climb={}",
            conv_dist,
            climb_dist
        );

        // But the actual XY coordinates of cutting moves should differ (reversed direction)
        // Find first cutting move after first plunge in each
        let conv_cuts: Vec<_> = conv_tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .collect();
        let climb_cuts: Vec<_> = climb_tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .collect();

        if conv_cuts.len() > 1 && climb_cuts.len() > 1 {
            // First cutting move after plunge should go in opposite directions
            let conv_dir_x = conv_cuts[0].target.x;
            let climb_dir_x = climb_cuts[0].target.x;
            // They won't be identical since one is reversed
            assert!(
                (conv_dir_x - climb_dir_x).abs() > 0.01
                    || (conv_cuts[0].target.y - climb_cuts[0].target.y).abs() > 0.01,
                "Climb and conventional should traverse contour in different directions"
            );
        }
    }

    #[test]
    fn test_pocket_no_cutting_above_surface() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = pocket_toolpath(&sq, &params);

        // No feed moves should be above cut_depth (except plunge which goes from safe_z to cut_depth)
        for i in 1..tp.moves.len() {
            if let MoveType::Linear { feed_rate } = tp.moves[i].move_type
                && (feed_rate - params.feed_rate).abs() < 1e-10
            {
                // This is a cutting move (not plunge) - must be at cut_depth
                assert!(
                    (tp.moves[i].target.z - params.cut_depth).abs() < 1e-10,
                    "Cutting move {} at z={} is not at cut_depth={}",
                    i,
                    tp.moves[i].target.z,
                    params.cut_depth
                );
            }
        }
    }

    #[test]
    fn test_pocket_contours_with_hole() {
        // 40×40 rect with 10×10 center hole — pocket should have more contours
        // than the same shape without a hole.
        let hole = vec![
            P2::new(15.0, 15.0),
            P2::new(15.0, 25.0),
            P2::new(25.0, 25.0),
            P2::new(25.0, 15.0),
        ]; // CW
        let poly_with_hole = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0).exterior,
            vec![hole],
        );
        let poly_no_hole = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);

        let contours_with = pocket_contours(&poly_with_hole, 2.0, 2.0);
        let contours_without = pocket_contours(&poly_no_hole, 2.0, 2.0);

        assert!(
            !contours_with.is_empty(),
            "Pocket with hole should produce contours"
        );

        // The polygon with a hole should produce more contours (the extra hole rings)
        assert!(
            contours_with.len() > contours_without.len(),
            "Pocket with hole should have more contours ({}) than without ({})",
            contours_with.len(),
            contours_without.len()
        );
    }

    #[test]
    fn test_pocket_toolpath_no_cuts_inside_island() {
        // 40×40 rect with 10×10 center hole
        let hole = vec![
            P2::new(15.0, 15.0),
            P2::new(15.0, 25.0),
            P2::new(25.0, 25.0),
            P2::new(25.0, 15.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0).exterior,
            vec![hole],
        );

        let params = PocketParams {
            tool_radius: 2.0,
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            climb: false,
        };
        let tp = pocket_toolpath(&poly, &params);

        // Verify no feed moves have XY inside the hole (with tool radius margin)
        let hole_margin = 1.0; // allow some tolerance for tool radius
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - params.feed_rate).abs() < 1e-6
            {
                let x = m.target.x;
                let y = m.target.y;
                // Inside the hole = x in (15+margin, 25-margin) and y in (15+margin, 25-margin)
                let inside_hole = x > 15.0 + hole_margin
                    && x < 25.0 - hole_margin
                    && y > 15.0 + hole_margin
                    && y < 25.0 - hole_margin;
                assert!(
                    !inside_hole,
                    "Feed move at ({:.1}, {:.1}) is inside the island",
                    x, y
                );
            }
        }
    }

    #[test]
    fn test_pocket_contours_l_shape() {
        // Non-convex L-shape
        let l_shape = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(30.0, 0.0),
            P2::new(30.0, 15.0),
            P2::new(15.0, 15.0),
            P2::new(15.0, 30.0),
            P2::new(0.0, 30.0),
        ]);
        let contours = pocket_contours(&l_shape, 2.0, 2.0);
        assert!(
            !contours.is_empty(),
            "L-shape pocket should produce contours"
        );

        let tp = pocket_toolpath(
            &l_shape,
            &PocketParams {
                tool_radius: 2.0,
                stepover: 2.0,
                cut_depth: -2.0,
                feed_rate: 800.0,
                plunge_rate: 400.0,
                safe_z: 5.0,
                climb: false,
            },
        );
        assert!(!tp.moves.is_empty());
    }
}
