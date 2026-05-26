//! F-036 — Per-segment adaptive feed modulation (algorithm core).
//!
//! This module implements the **pure algorithm** Fusion HSM calls "adaptive
//! feed control": walk a toolpath, compute a target feed for every cutting
//! move that keeps chip thickness inside the LUT band, and write the
//! per-move feed back into the IR.
//!
//! **Scope (F-036 Piece A + B only).** This finding originally bundled four
//! pieces (IR-feed check, algorithm, G-code emission, feature-flag
//! plumbing). This module covers Piece A (IR already carries per-move
//! feed — verified) + Piece B (the modulation function + its unit tests).
//! Piece C (G-code emitter changes) and Piece D (feature flag + production
//! wiring) are deferred to sub-findings **F-036a** (G-code emission of
//! per-move F-words) and **F-036b** (`SimulationOptions` /
//! `OperationConfig` flag plumbing + the seven acceptance tests from the
//! finding file). Nothing in production calls
//! [`adaptive_feed_modulate`] yet — it is reachable only from this
//! module's unit tests.
//!
//! **Why decouple from the simulator?** The brief suggested taking a
//! `&[SimulationCutSample]` slice and filtering by `move_index`. That works
//! but couples this algorithm to the simulator's wire shape and makes
//! unit-testing tedious (every test stages a `SimulationCutTrace`). The
//! decoupled form here accepts a `&[PerMoveEngagement]` summary keyed by
//! move index; the caller (F-036b production wiring) collapses simulator
//! samples into that summary in one pass before invoking the modulator.
//! Synthetic tests build the summary directly from known geometry.
//!
//! **What this module does NOT do.**
//!
//! - It does **not** read or write G-code. The IR carries per-move
//!   `feed_rate`; G-code emission of `F<rate>` on every feed change is
//!   F-036a.
//! - It does **not** call the simulator. Engagement summaries are an
//!   input.
//! - It does **not** modulate `MoveType::Rapid`, retract moves
//!   (`MoveIntent::Retract`), or drilling plunges (`MoveIntent::Drilling`,
//!   `MoveIntent::EntryPlunge`). The chip-thinning correction is only
//!   well-defined for lateral / arc / helix engagement.
//! - It does **not** introduce per-segment junction-velocity smoothing.
//!   The kinematics integrator
//!   ([`crate::machine_kinematics::predicted_feeds_for_toolpath`]) caps
//!   modulated feeds at the machine-achievable peak velocity, so a
//!   modulated `F<rate>` the planner can't physically reach is downgraded
//!   to one it can.
//!
//! See `planning/feed_modulation_roadmap.md` and
//! `planning/acceptance_loop/findings/F-036-per-segment-feed-modulation.md`
//! for the workstream narrative and the deferred acceptance-test list.

use crate::machine_kinematics::{predicted_feeds_for_toolpath, MachineKinematics};
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

/// Chipload band (`mm/tooth`) sourced from the vendor LUT for a given
/// tool / material pair.
///
/// `min` is the rubbing / heat-burn floor (below this the tooth scrapes
/// instead of slicing; in wood the workpiece scorches). `max` is the
/// breakage / over-load ceiling. The modulator targets the band's
/// midpoint and clamps modulated feeds so the **commanded** chipload
/// stays inside `[min, max]` after the engagement correction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiploadBand {
    /// Minimum chipload (mm per tooth). Must be > 0 and ≤ `max`.
    pub min_mm_per_tooth: f64,
    /// Maximum chipload (mm per tooth). Must be ≥ `min`.
    pub max_mm_per_tooth: f64,
}

impl ChiploadBand {
    /// Construct a chipload band. Returns `None` if either value is
    /// non-finite, `min` is ≤ 0, or `max` < `min`.
    pub fn new(min_mm_per_tooth: f64, max_mm_per_tooth: f64) -> Option<Self> {
        if !min_mm_per_tooth.is_finite()
            || !max_mm_per_tooth.is_finite()
            || min_mm_per_tooth <= 0.0
            || max_mm_per_tooth < min_mm_per_tooth
        {
            return None;
        }
        Some(Self {
            min_mm_per_tooth,
            max_mm_per_tooth,
        })
    }

    /// Geometric midpoint of the band — the modulator's chipload target.
    /// Geometric (not arithmetic) so the target sits proportionally
    /// between min and max regardless of band width.
    #[inline]
    pub fn mid_mm_per_tooth(&self) -> f64 {
        (self.min_mm_per_tooth * self.max_mm_per_tooth).sqrt()
    }
}

/// Per-move engagement summary the modulator consumes.
///
/// One entry per cutting move in the toolpath (Rapids carry no
/// engagement and the modulator skips them). The two fractions match
/// the simulator's `Engagement` axes: `radial_woc_fraction` is the
/// cylinder-side radial WOC fraction (0.0 = air, 1.0 = full slot);
/// `axial_doc_fraction` is the cutter's flute-engaged-height fraction.
/// Both are time-weighted means across the move's samples.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PerMoveEngagement {
    /// Mean radial WOC fraction across the move's cutting samples.
    /// `0.0` is acceptable (the modulator treats the move as air and
    /// leaves its feed at the commanded value).
    pub radial_woc_fraction: f64,
    /// Mean axial DOC fraction across the move's cutting samples.
    /// Currently used only to flag "no engagement" (≤ 1e-6 → air).
    /// Reserved for the deflection-aware modulation extension.
    pub axial_doc_fraction: f64,
}

/// Static inputs the modulator needs to convert engagement into feed.
///
/// Kept tool/material/machine-agnostic at the struct level so a caller
/// can stage a synthetic context for a unit test without instantiating
/// the full `Tool` / `Material` / `Machine` graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModulationContext<'a> {
    /// Spindle speed at which the toolpath is commanded (rev/min).
    /// Must be > 0.
    pub spindle_rpm: f64,
    /// Flute count of the active cutter. Must be ≥ 1.
    pub flute_count: u32,
    /// Machine's `max_feed_mm_min` hard cap (passed to the kinematics
    /// integrator and used to clamp modulated feeds).
    pub max_feed_mm_min: f64,
    /// Rapid-move feed (mm/min) the machine emits for `G0`. Used only
    /// to thread the kinematics integrator correctly; rapids
    /// themselves are not modulated.
    pub rapid_feed_mm_min: f64,
    /// LUT chipload band (mm/tooth).
    pub chipload_band: ChiploadBand,
    /// Machine kinematics — drives the `predicted_feeds_for_toolpath`
    /// per-move achievable-velocity cap.
    pub kinematics: &'a MachineKinematics,
}

/// Errors the modulator can return for malformed inputs. Kept small so
/// the workspace's `result_large_err` lint stays happy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationError {
    /// `engagements` length didn't match `toolpath.moves.len()`.
    EngagementLengthMismatch,
    /// `spindle_rpm`, `flute_count`, `max_feed_mm_min`, or `rapid_feed_mm_min`
    /// was non-positive or non-finite.
    InvalidContext,
}

/// Return `true` when the modulator should leave this move's feed alone.
///
/// Three classes:
/// - rapids (no feed in the IR to begin with),
/// - retract / drilling / entry-plunge intents (chip-thinning model
///   doesn't apply to pure-vertical or air-traverse moves),
/// - moves the caller flagged with `Unknown` intent (legacy
///   generators) — these are still modulated when the engagement
///   summary reports a non-trivial radial WOC.
fn should_skip_modulation(move_type: MoveType, intent: MoveIntent) -> bool {
    if matches!(move_type, MoveType::Rapid) {
        return true;
    }
    matches!(
        intent,
        MoveIntent::Retract | MoveIntent::Drilling | MoveIntent::EntryPlunge
    )
}

/// Compute the target feed (mm/min) for one cutting move.
///
/// Algorithm:
///
/// 1. Start from the chipload band's geometric mid-point as the
///    chipload target.
/// 2. Apply the chip-thinning correction:
///    `effective_chip = commanded_chip × sqrt(radial_woc_fraction)`.
///    The radial-WOC < 1 path produces a thinner chip than the feed
///    would naively give, so to land **on** the mid-band the feed
///    must be scaled by `1 / sqrt(radial_woc_fraction)`. At
///    `radial_woc_fraction == 1` (full slot) the correction is unity.
/// 3. Convert to feed: `feed = target_chipload × rpm × flutes`.
/// 4. Cap at:
///    - `max_feed_mm_min` (the machine's hard cap),
///    - the move's predicted achievable feed from the kinematics
///      integrator (so the modulator never asks the planner for a
///      feed it can't reach),
///    - `band.max × rpm × flutes` (never command above the
///      breakage ceiling).
/// 5. Floor at `band.min × rpm × flutes` (rubbing protection).
/// 6. If the radial WOC is effectively zero (air move that slipped
///    through), return the commanded feed unchanged.
fn target_feed_for_move(
    commanded_feed_mm_min: f64,
    engagement: PerMoveEngagement,
    predicted_cap_mm_min: f64,
    ctx: &ModulationContext<'_>,
) -> f64 {
    if engagement.radial_woc_fraction <= 1e-6 && engagement.axial_doc_fraction <= 1e-6 {
        return commanded_feed_mm_min;
    }
    let woc = engagement.radial_woc_fraction.clamp(1e-3, 1.0);
    let thinning = woc.sqrt().max(1e-6);
    let target_chipload = ctx.chipload_band.mid_mm_per_tooth() / thinning;
    let flutes = ctx.flute_count.max(1) as f64;
    let base_feed = target_chipload * ctx.spindle_rpm * flutes;

    let band_floor = ctx.chipload_band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
    let band_ceiling = ctx.chipload_band.max_mm_per_tooth * ctx.spindle_rpm * flutes;

    let cap = ctx
        .max_feed_mm_min
        .min(band_ceiling)
        .min(predicted_cap_mm_min.max(band_floor));
    // If the cap sits below the floor (rare: machine `max_feed` was
    // configured below `band_floor`), the floor wins — emitting below
    // the rubbing threshold is never safe, and `clamp(floor, cap < floor)`
    // would panic. Drop down to the machine cap instead.
    if cap < band_floor {
        return cap.max(0.0);
    }
    base_feed.clamp(band_floor, cap)
}

/// Apply per-segment adaptive feed modulation to `toolpath`.
///
/// For every cutting move whose intent is not retract / drilling /
/// entry-plunge, compute a new target feed from the move's engagement
/// summary + the LUT chipload band, capped at the machine's predicted
/// achievable feed. Rapid moves and skipped-intent moves keep their
/// existing feed (or absence of one).
///
/// Returns the number of moves whose feed was actually changed — useful
/// for unit tests asserting "at least N moves were modulated."
///
/// **The function is side-effect-free apart from mutating per-move
/// `feed_rate` fields on `toolpath`.** It does not call the simulator
/// (engagement summary is an input) and does not interact with G-code
/// emission (the IR's `feed_rate` field is the channel through which
/// the post-processor sees the modulated feed). G-code emission is
/// F-036a.
pub fn adaptive_feed_modulate(
    toolpath: &mut Toolpath,
    engagements: &[PerMoveEngagement],
    ctx: &ModulationContext<'_>,
) -> Result<usize, ModulationError> {
    if engagements.len() != toolpath.moves.len() {
        return Err(ModulationError::EngagementLengthMismatch);
    }
    if !ctx.spindle_rpm.is_finite()
        || ctx.spindle_rpm <= 0.0
        || ctx.flute_count == 0
        || !ctx.max_feed_mm_min.is_finite()
        || ctx.max_feed_mm_min <= 0.0
        || !ctx.rapid_feed_mm_min.is_finite()
        || ctx.rapid_feed_mm_min <= 0.0
    {
        return Err(ModulationError::InvalidContext);
    }

    // Predicted achievable feed per move under the machine's accel /
    // junction limits. The standard
    // `predicted_feeds_for_toolpath(toolpath, ...)` caps each move at
    // its **commanded** feed — useful for "did the controller actually
    // reach commanded?" but unhelpful here because the modulator may
    // want to raise feed above commanded on a long straight. To get
    // the move's **geometric** achievable feed we rebuild a synthetic
    // toolpath where every cutting move is commanded at
    // `min(max_feed, band_ceiling × RPM × flutes)` and integrate that.
    let band_ceiling_feed = ctx.chipload_band.max_mm_per_tooth
        * ctx.spindle_rpm
        * ctx.flute_count.max(1) as f64;
    let probe_feed = ctx.max_feed_mm_min.min(band_ceiling_feed).max(1e-3);
    let mut probe = toolpath.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = match m.move_type {
            MoveType::Linear { .. } => MoveType::Linear {
                feed_rate: probe_feed,
            },
            MoveType::ArcCW { i, j, .. } => MoveType::ArcCW {
                i,
                j,
                feed_rate: probe_feed,
            },
            MoveType::ArcCCW { i, j, .. } => MoveType::ArcCCW {
                i,
                j,
                feed_rate: probe_feed,
            },
            MoveType::Rapid => MoveType::Rapid,
        };
    }
    let predicted = predicted_feeds_for_toolpath(
        &probe,
        ctx.kinematics,
        ctx.max_feed_mm_min,
        ctx.rapid_feed_mm_min,
    );

    let mut changed = 0usize;
    #[allow(clippy::indexing_slicing)]
    // SAFETY: `i` bounded by `engagements.len()`, which we just verified
    // equals `toolpath.moves.len()`.
    for (i, &engagement) in engagements.iter().enumerate() {
        let move_intent = toolpath.moves[i].intent;
        let move_type = toolpath.moves[i].move_type;
        if should_skip_modulation(move_type, move_intent) {
            continue;
        }
        let Some(commanded) = move_type.feed_rate() else {
            continue;
        };
        let predicted_cap = predicted
            .get(&i)
            .copied()
            .unwrap_or(ctx.max_feed_mm_min)
            .max(1e-3);
        let new_feed = target_feed_for_move(commanded, engagement, predicted_cap, ctx);
        if (new_feed - commanded).abs() > 1e-6 {
            // Mutate the per-move feed field in place. `MoveType` is
            // `Copy`, so we rebuild it with the new feed and replace.
            let new_move_type = match move_type {
                MoveType::Linear { .. } => MoveType::Linear { feed_rate: new_feed },
                MoveType::ArcCW { i: ai, j: aj, .. } => MoveType::ArcCW {
                    i: ai,
                    j: aj,
                    feed_rate: new_feed,
                },
                MoveType::ArcCCW { i: ai, j: aj, .. } => MoveType::ArcCCW {
                    i: ai,
                    j: aj,
                    feed_rate: new_feed,
                },
                MoveType::Rapid => MoveType::Rapid,
            };
            toolpath.moves[i].move_type = new_move_type;
            changed += 1;
        }
    }
    Ok(changed)
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
    use crate::geo::P3;
    use crate::toolpath::Toolpath;

    fn shapeoko() -> MachineKinematics {
        MachineKinematics::shapeoko_xxl_stock()
    }

    fn make_ctx<'a>(k: &'a MachineKinematics, band: ChiploadBand) -> ModulationContext<'a> {
        ModulationContext {
            spindle_rpm: 18_000.0,
            flute_count: 2,
            max_feed_mm_min: 4000.0,
            rapid_feed_mm_min: 5000.0,
            chipload_band: band,
            kinematics: k,
        }
    }

    fn band() -> ChiploadBand {
        ChiploadBand::new(0.02, 0.08).unwrap()
    }

    /// A simple toolpath: rapid to start, then N long colinear cutting
    /// moves of equal length. The kinematics integrator should let
    /// every cutting move reach its commanded feed (no corners), so
    /// the predicted-cap never trims modulation.
    fn straight_toolpath(n_cuts: usize, feed_mm_min: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        for i in 0..n_cuts {
            let x = (i + 1) as f64 * 50.0;
            tp.feed_to_with_intent(
                P3::new(x, 0.0, -2.0),
                feed_mm_min,
                MoveIntent::ClearingCut,
            );
        }
        tp
    }

    /// A corner-heavy toolpath: rapid, then short alternating-direction
    /// cutting moves. The kinematics integrator drops predicted feed
    /// below commanded on tight corners.
    fn corner_heavy_toolpath(feed_mm_min: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        // Series of 2 mm-long zig-zag cuts at 90° to each other.
        let mut x = 0.0;
        let mut y = 0.0;
        for i in 0..8 {
            if i % 2 == 0 {
                x += 2.0;
            } else {
                y += 2.0;
            }
            tp.feed_to_with_intent(
                P3::new(x, y, -2.0),
                feed_mm_min,
                MoveIntent::ClearingCut,
            );
        }
        tp
    }

    #[test]
    fn chipload_band_rejects_invalid() {
        assert!(ChiploadBand::new(0.0, 0.05).is_none());
        assert!(ChiploadBand::new(0.05, 0.02).is_none());
        assert!(ChiploadBand::new(f64::NAN, 0.05).is_none());
        assert!(ChiploadBand::new(-0.01, 0.05).is_none());
    }

    #[test]
    fn chipload_band_mid_is_geometric_mean() {
        let b = ChiploadBand::new(0.02, 0.08).unwrap();
        // sqrt(0.02 * 0.08) = sqrt(0.0016) = 0.04
        assert!((b.mid_mm_per_tooth() - 0.04).abs() < 1e-9);
    }

    #[test]
    fn engagement_length_mismatch_errors() {
        let mut tp = straight_toolpath(3, 1500.0);
        // Off-by-one engagement vector.
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len() - 1];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let err = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err();
        assert_eq!(err, ModulationError::EngagementLengthMismatch);
    }

    #[test]
    fn invalid_context_errors() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.spindle_rpm = 0.0;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
        let mut ctx = make_ctx(&k, band());
        ctx.flute_count = 0;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
        let mut ctx = make_ctx(&k, band());
        ctx.max_feed_mm_min = f64::NAN;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
    }

    /// All-air engagement: feed must stay byte-identical (no modulation
    /// triggered). This is the "flag-off equivalent" invariant for the
    /// algorithm itself — when there's no engagement to optimise, the
    /// modulator must be a no-op.
    #[test]
    fn zero_engagement_leaves_feed_unchanged() {
        let mut tp = straight_toolpath(4, 1500.0);
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let changed = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(changed, 0, "no engagement should produce no modulation");
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!((f - 1500.0).abs() < 1e-9, "feed unchanged: got {}", f);
            }
        }
    }

    /// Skipped intents (Retract, Drilling, EntryPlunge) must keep their
    /// commanded feed even when their engagement summary says otherwise.
    /// This is the "don't modulate plunges" rule from the brief.
    #[test]
    fn skipped_intents_keep_commanded_feed() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -3.0), 300.0, MoveIntent::EntryPlunge);
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -6.0), 300.0, MoveIntent::Drilling);
        tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 1500.0, MoveIntent::Retract);
        // Pretend all three saw heavy engagement (so the modulator would
        // change them if it weren't skipping by intent).
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
        ];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let changed = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(
            changed, 0,
            "EntryPlunge / Drilling / Retract must be skipped"
        );
        assert!((tp.moves[1].move_type.feed_rate().unwrap() - 300.0).abs() < 1e-9);
        assert!((tp.moves[2].move_type.feed_rate().unwrap() - 300.0).abs() < 1e-9);
        assert!((tp.moves[3].move_type.feed_rate().unwrap() - 1500.0).abs() < 1e-9);
    }

    /// Full-slot engagement (radial WOC = 1.0): no chip-thinning
    /// correction. The modulator targets `mid_band × RPM × flutes`
    /// directly — that's `0.04 × 18000 × 2 = 1440 mm/min`. With
    /// commanded 1500, the modulator must drop feed to ~1440.
    #[test]
    fn full_slot_targets_mid_band_feed() {
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
        ];
        let k = shapeoko();
        let ctx = make_ctx(&k, band()); // mid = 0.04
        let changed = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert!(changed >= 1);
        for m in &tp.moves {
            if !matches!(m.move_type, MoveType::Rapid) {
                let f = m.move_type.feed_rate().unwrap();
                // 0.04 × 18000 × 2 = 1440.
                assert!(
                    (f - 1440.0).abs() < 5.0,
                    "expected ~1440 mm/min on full slot, got {}",
                    f
                );
            }
        }
    }

    /// Light engagement (radial WOC = 0.1): chip-thinning correction is
    /// `1 / sqrt(0.1) ≈ 3.16×`. Target chipload becomes
    /// `0.04 × 3.16 ≈ 0.1265 mm/tooth`. That's above the band ceiling
    /// (0.08) so the modulator must clamp to ceiling-feed:
    /// `0.08 × 18000 × 2 = 2880 mm/min`. Confirms the
    /// "never command above the breakage ceiling" clamp.
    #[test]
    fn light_engagement_clamps_at_band_ceiling() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
            },
        ];
        let k = shapeoko();
        let ctx = make_ctx(&k, band()); // ceiling = 0.08
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        let f = tp.moves[1].move_type.feed_rate().unwrap();
        // 0.08 × 18000 × 2 = 2880.
        assert!(
            (f - 2880.0).abs() < 5.0,
            "expected band-ceiling clamp at 2880, got {}",
            f
        );
    }

    /// `band.max × RPM × flutes` exceeds `max_feed_mm_min`: the
    /// machine-cap clamp must beat the band-ceiling clamp. With
    /// `max_feed_mm_min = 500` and ceiling-feed = 2880, the modulated
    /// feed should land at 500.
    #[test]
    fn machine_max_feed_cap_wins() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
            },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.max_feed_mm_min = 500.0;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!(f <= 500.0 + 1e-6, "max_feed cap violated: {}", f);
            }
        }
    }

    /// Modulated feed must never fall below
    /// `band.min × RPM × flutes`. Engineered scenario: tiny mid-band
    /// (rubbing floor) on a heavy-engagement move. The chip-thinning
    /// correction at WOC=1 would target the geometric mid, which sits
    /// well above the floor — so the floor doesn't fire here. The
    /// "floor protection" check matters when a future caller passes
    /// a band whose mid lands below the floor; we assert the floor
    /// holds by passing a degenerate band with `min == max == 0.05` and
    /// confirming the modulated feed equals `0.05 × RPM × flutes`.
    #[test]
    fn modulation_never_emits_below_min_chipload() {
        let mut tp = straight_toolpath(3, 4000.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
            },
        ];
        let k = shapeoko();
        let degenerate = ChiploadBand::new(0.05, 0.05).unwrap();
        let ctx = make_ctx(&k, degenerate);
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        // 0.05 × 18000 × 2 = 1800 mm/min — both floor and ceiling.
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!(
                    f >= 1800.0 - 1e-6,
                    "floor protection violated: {} < 1800",
                    f
                );
                assert!(
                    f <= 1800.0 + 1e-6,
                    "ceiling protection violated: {} > 1800",
                    f
                );
            }
        }
    }

    /// Corner-heavy toolpath: kinematics integrator drops predicted
    /// feed below commanded in tight corners. The modulator must
    /// honor that cap — modulated feed on a corner move should not
    /// exceed the predicted achievable feed for that move.
    #[test]
    fn modulation_respects_predicted_feed_cap() {
        let mut tp = corner_heavy_toolpath(4000.0);
        let engagements: Vec<_> = (0..tp.moves.len())
            .map(|_| PerMoveEngagement {
                radial_woc_fraction: 0.1, // light: would want feed > commanded
                axial_doc_fraction: 1.0,
            })
            .collect();
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let predicted_pre = predicted_feeds_for_toolpath(&tp, &k, ctx.max_feed_mm_min, 5000.0);
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        for (idx, m) in tp.moves.iter().enumerate() {
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            let f = m.move_type.feed_rate().unwrap();
            if let Some(&cap) = predicted_pre.get(&idx) {
                // Allow a tiny slack for the predicted floor (`max(cap, band_floor)`)
                // — the modulator's cap is `min(max_feed, ceiling, max(predicted, floor))`,
                // so it can land slightly above `cap` when `band_floor > cap`.
                let band_floor = ctx.chipload_band.min_mm_per_tooth
                    * ctx.spindle_rpm
                    * ctx.flute_count as f64;
                let effective_cap = cap.max(band_floor);
                assert!(
                    f <= effective_cap + 1.0,
                    "move {} feed {} exceeds predicted cap {} (band_floor={})",
                    idx,
                    f,
                    cap,
                    band_floor,
                );
            }
        }
    }

    /// Per-segment variation invariant: corner-heavy + heterogeneous
    /// engagement → modulated path must have **at least two distinct
    /// feed values** (i.e. modulation actually varies feed, not just
    /// uniformly drops it). This is the per-segment-variation bar
    /// the finding's acceptance test asks for, expressed at the
    /// algorithm layer.
    #[test]
    fn modulation_produces_per_segment_feed_variation() {
        let mut tp = corner_heavy_toolpath(2000.0);
        // Alternate heavy / light engagement to force per-segment variation.
        let engagements: Vec<_> = (0..tp.moves.len())
            .map(|i| {
                if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                    PerMoveEngagement::default()
                } else if i % 2 == 0 {
                    PerMoveEngagement {
                        radial_woc_fraction: 1.0,
                        axial_doc_fraction: 1.0,
                    }
                } else {
                    PerMoveEngagement {
                        radial_woc_fraction: 0.2,
                        axial_doc_fraction: 1.0,
                    }
                }
            })
            .collect();
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        let mut feeds: Vec<f64> = tp
            .moves
            .iter()
            .filter_map(|m| m.move_type.feed_rate())
            .collect();
        feeds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        feeds.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        assert!(
            feeds.len() >= 2,
            "expected ≥ 2 distinct feeds across modulated path, got {:?}",
            feeds
        );
    }
}
