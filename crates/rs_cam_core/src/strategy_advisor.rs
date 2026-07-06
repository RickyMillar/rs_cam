//! Strategy advisor — suggest the clearing strategy, don't make the user
//! pick (`planning/STRATEGY_ADVISOR_2026-06-17.md`).
//!
//! Given each candidate clearing strategy's **load-limited** toolpath
//! (params already backed off to the deflection / power limits by the
//! Suggest pass), plus the machine, this picks the strategy that minimises
//! **wall-clock at the load limit** and surfaces the binding-constraint
//! "regime" (tool-limited / machine-limited) as the human-readable *why*.
//!
//! ## The insight the advisor encodes
//!
//! The naïve rule "tool-limited ⇒ spiral / constant-engagement" is wrong
//! on a low-acceleration machine. The honest objective is
//!
//! > minimise wall-clock `T`, subject to peak tool load ≤ limit.
//!
//! A backed-off [`ContourParallel`](ClearingStrategy::ContourParallel)
//! lays down more path but holds commanded feed along its long straights;
//! a [`ContourSpiral`](ClearingStrategy::ContourSpiral) lays down less
//! path but the machine decelerates into every loop. Which wins is set by
//! **acceleration**, so the decision is *measured*, not looked up: run the
//! real accel-aware integrator
//! ([`compute_cycle_time`](crate::machine_kinematics::compute_cycle_time))
//! over each candidate's actual toolpath at the machine's
//! [`effective_kinematics`](crate::machine::MachineProfile::effective_kinematics)
//! and compare. On a rigid VMC the spiral's shorter path wins; on a belt
//! router the parallel's cruising straights win — the same op, opposite
//! call, decided by the accel number.
//!
//! ## Scope
//!
//! This module is the *decision core*: it consumes candidate toolpaths and
//! ranks them. Generating each candidate (running Suggest's params-optimise
//! for a strategy, then planning the toolpath) is heavier orchestration
//! that lives with the planner / GUI worker; surfacing the recommendation
//! as an accept-or-override advisory in the op UI is a follow-up (doc §
//! "Build order" items 3–4). Keeping the decision pure makes the
//! accel-dependence directly testable (see the sentry below).

use crate::compute::operation_configs::ClearingStrategy;
use crate::machine::MachineProfile;
use crate::machine_kinematics::compute_cycle_time;
use crate::toolpath::Toolpath;

/// Which constraint bound a candidate's load-limited params — the
/// human-readable "regime" surfaced as the recommendation's *why*. This
/// is a property of the tool + material + cut, so candidates on the same
/// operation usually agree; the advisor reports the *chosen* candidate's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadRegime {
    /// Deflection (the tool) is the binding limit — the cut is tool-limited.
    ToolLimited,
    /// Power / feed (the machine) is the binding limit — machine-limited.
    MachineLimited,
    /// Neither limit is stressed — a light cut.
    Unconstrained,
}

/// A clearing strategy paired with the toolpath it produces at its
/// load-limited params. The caller (planning orchestration) builds each
/// candidate by running Suggest for the strategy and planning the path;
/// the advisor measures and ranks them.
pub struct StrategyCandidate<'a> {
    pub strategy: ClearingStrategy,
    /// Toolpath generated at the strategy's load-limited params.
    pub toolpath: &'a Toolpath,
    /// Which constraint bound the params for this candidate.
    pub regime: LoadRegime,
    /// `true` when geometry forces this strategy — a deep narrow feature a
    /// conventional path can only reach by full-width slotting or gouging,
    /// where trochoidal entry is the only safe option. A forced candidate
    /// wins regardless of wall-clock.
    pub geometry_forced: bool,
}

/// One candidate's measured standing in the ranking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankedStrategy {
    pub strategy: ClearingStrategy,
    /// Accel-aware wall-clock estimate (seconds) for this candidate's
    /// toolpath on the target machine.
    pub wall_clock_s: f64,
    pub regime: LoadRegime,
    pub geometry_forced: bool,
}

/// The advisor's output: the chosen strategy, the regime that explains it,
/// a one-line *why*, and every measurable candidate ranked by wall-clock.
#[derive(Debug, Clone)]
pub struct StrategyRecommendation {
    pub chosen: ClearingStrategy,
    pub regime: LoadRegime,
    pub reason: String,
    /// Candidates sorted ascending by wall-clock (fastest first).
    pub ranked: Vec<RankedStrategy>,
    /// How many times faster the chosen strategy is than the next-best
    /// (`runner_up_s / chosen_s`, so `> 1.0` means the choice is faster).
    /// `1.0` when there is no runner-up, or when a slower geometry-forced
    /// candidate was chosen over a faster one.
    pub time_ratio_vs_runner_up: f64,
}

/// Constant-engagement ("spiral-class") strategies carry feed through a
/// continuous, even-load path; conventional strategies trade straighter
/// motion for more path. This drives only the recommendation's *why*
/// string — the *choice* is always the measured wall-clock minimum.
fn is_constant_engagement(strategy: ClearingStrategy) -> bool {
    matches!(
        strategy,
        ClearingStrategy::ContourSpiral | ClearingStrategy::AgentSearch
    )
}

/// Stable display label for the recommendation text.
fn strategy_label(strategy: ClearingStrategy) -> &'static str {
    match strategy {
        ClearingStrategy::AgentSearch => "Agent Search",
        ClearingStrategy::ContourParallel => "Contour Parallel",
        ClearingStrategy::Adaptive => "Adaptive",
        ClearingStrategy::ContourSpiral => "Contour Spiral",
    }
}

/// Pick the clearing strategy that minimises wall-clock at the load limit.
///
/// Each candidate is timed through the real accel-aware integrator on the
/// machine's [`effective_kinematics`](MachineProfile::effective_kinematics);
/// the fastest measurable candidate wins unless a candidate is
/// `geometry_forced` (then it wins regardless of time — only it can clear
/// the feature safely). Candidates whose toolpath is empty / degenerate
/// (non-positive estimated time) are dropped.
///
/// Returns `None` when no candidate has a measurable toolpath.
pub fn recommend_strategy(
    candidates: &[StrategyCandidate<'_>],
    machine: &MachineProfile,
) -> Option<StrategyRecommendation> {
    let kinematics = machine.effective_kinematics();
    // Rapids run at the travel rate; cutting moves carry their own feed,
    // clamped by the same travel cap the controller would apply.
    let max_feed = machine.max_feed_mm_min;
    let rapid_feed = machine.max_feed_mm_min;

    let mut ranked: Vec<RankedStrategy> = candidates
        .iter()
        .map(|c| RankedStrategy {
            strategy: c.strategy,
            wall_clock_s: compute_cycle_time(c.toolpath, &kinematics, max_feed, rapid_feed),
            regime: c.regime,
            geometry_forced: c.geometry_forced,
        })
        // Drop empty / degenerate toolpaths — a 0 s estimate is "not
        // measurable", not "instant", and must never win the argmin.
        .filter(|r| r.wall_clock_s.is_finite() && r.wall_clock_s > 0.0)
        .collect();

    if ranked.is_empty() {
        return None;
    }

    ranked.sort_by(|a, b| a.wall_clock_s.total_cmp(&b.wall_clock_s));

    // Geometry override: a forced strategy is the only safe way to clear
    // the feature, so it wins regardless of where it sits on time.
    let chosen = ranked
        .iter()
        .find(|r| r.geometry_forced)
        .copied()
        // SAFETY: `ranked` is non-empty (checked above), so `first` is Some.
        .unwrap_or_else(|| {
            #[allow(clippy::expect_used)]
            // SAFETY: non-empty per the guard above.
            *ranked.first().expect("ranked is non-empty")
        });

    // Runner-up = fastest candidate that isn't the chosen one.
    let runner_up = ranked
        .iter()
        .find(|r| r.strategy != chosen.strategy)
        .copied();
    let time_ratio_vs_runner_up = match runner_up {
        Some(r) if chosen.wall_clock_s > 0.0 => r.wall_clock_s / chosen.wall_clock_s,
        _ => 1.0,
    };

    let reason = build_reason(chosen, runner_up, time_ratio_vs_runner_up);

    Some(StrategyRecommendation {
        chosen: chosen.strategy,
        regime: chosen.regime,
        reason,
        ranked,
        time_ratio_vs_runner_up,
    })
}

/// Compose the one-line *why* from the binding regime, whether the chosen
/// strategy is constant-engagement, and the measured speed margin.
fn build_reason(
    chosen: RankedStrategy,
    runner_up: Option<RankedStrategy>,
    time_ratio_vs_runner_up: f64,
) -> String {
    let label = strategy_label(chosen.strategy);
    let core = if chosen.geometry_forced {
        format!(
            "geometry forces {label}: only constant-engagement clearing reaches this feature \
             without full-width slotting"
        )
    } else {
        match (chosen.regime, is_constant_engagement(chosen.strategy)) {
            (LoadRegime::ToolLimited, true) => format!(
                "tool-limited; this machine's acceleration carries feed through {label}'s \
                 constant-load path"
            ),
            (LoadRegime::ToolLimited, false) => format!(
                "tool-limited, but backing off {label} beats the spiral's per-loop deceleration \
                 on this machine's acceleration"
            ),
            (LoadRegime::MachineLimited, _) => format!(
                "machine-limited; constant engagement buys no extra removal rate here, so {label} \
                 is fastest"
            ),
            (LoadRegime::Unconstrained, _) => format!(
                "neither tool nor machine is stressed; {label}'s calmer motion suits the machine"
            ),
        }
    };

    // Only quote a speed margin when it's both real and meaningful, and
    // when the choice is actually the faster one (a geometry-forced pick
    // may be slower — don't claim a speedup it doesn't have).
    match runner_up {
        Some(r) if time_ratio_vs_runner_up > 1.05 => {
            format!(
                "{core} (~{time_ratio_vs_runner_up:.1}× faster than {})",
                strategy_label(r.strategy)
            )
        }
        _ => core,
    }
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
    use crate::machine_kinematics::MachineKinematics;
    use crate::toolpath::Toolpath;

    /// A long-straight raster: 8 passes of 250 mm along X stepping in Y.
    /// Few junctions per unit path — the machine cruises the straights and
    /// only decelerates at the 7 reversals. Total path ≈ 2000 mm.
    fn parallel_path(feed: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, -1.0));
        let mut y = 0.0;
        for pass in 0..8 {
            let x_end = if pass % 2 == 0 { 250.0 } else { 0.0 };
            tp.feed_to(P3::new(x_end, y, -1.0), feed);
            y += 5.0;
            tp.feed_to(P3::new(x_end, y, -1.0), feed);
        }
        tp
    }

    /// A tight zigzag standing in for a constant-engagement spiral: 300
    /// short 4 mm chords that reverse direction sharply at every junction,
    /// so the corner model drops junction velocity to near zero each time.
    /// Total path ≈ 1200 mm — 40 % SHORTER than the parallel raster, but
    /// every one of its ~300 junctions forces a full accel/decel cycle.
    fn spiral_path(feed: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, -1.0));
        let seg = 4.0;
        let mut x = 0.0;
        for i in 0..300 {
            // Sawtooth in Y with steady X drift: each junction is a sharp
            // (~90°+) reversal in the Y direction.
            let y = if i % 2 == 0 { 3.0 } else { 0.0 };
            x += seg * 0.25; // small forward drift so the chord length ≈ seg
            tp.feed_to(P3::new(x, y, -1.0), feed);
        }
        tp
    }

    fn machine_with_accel(accel_mm_s2: f64) -> MachineProfile {
        MachineProfile {
            max_feed_mm_min: 10_000.0,
            kinematics: Some(MachineKinematics {
                acceleration_mm_s2: accel_mm_s2,
                ..MachineKinematics::default()
            }),
            ..MachineProfile::default()
        }
    }

    /// The doc's core sentry: the winner FLIPS with machine acceleration.
    /// Same two candidates (a long-straight parallel raster vs a shorter
    /// but corner-dense spiral); on a low-accel belt router the parallel's
    /// cruising straights win, on a rigid high-accel machine the spiral's
    /// shorter path wins. Pins the accel-dependence so the advisor can
    /// never regress to a naïve "tool-limited ⇒ spiral" lookup.
    #[test]
    fn winner_flips_with_machine_acceleration() {
        let feed = 3000.0;
        let parallel = parallel_path(feed);
        let spiral = spiral_path(feed);

        let candidates = [
            StrategyCandidate {
                strategy: ClearingStrategy::ContourParallel,
                toolpath: &parallel,
                regime: LoadRegime::ToolLimited,
                geometry_forced: false,
            },
            StrategyCandidate {
                strategy: ClearingStrategy::ContourSpiral,
                toolpath: &spiral,
                regime: LoadRegime::ToolLimited,
                geometry_forced: false,
            },
        ];

        // Low-accel belt router: parallel wins.
        let low = machine_with_accel(80.0);
        let rec_low = recommend_strategy(&candidates, &low).unwrap();
        assert_eq!(
            rec_low.chosen,
            ClearingStrategy::ContourParallel,
            "low accel: backed-off parallel should win (spiral decelerates into every loop); reason: {}",
            rec_low.reason
        );

        // High-accel rigid machine: spiral wins (its shorter path is no
        // longer penalised by per-corner deceleration).
        let high = machine_with_accel(5000.0);
        let rec_high = recommend_strategy(&candidates, &high).unwrap();
        assert_eq!(
            rec_high.chosen,
            ClearingStrategy::ContourSpiral,
            "high accel: shorter-path spiral should win; reason: {}",
            rec_high.reason
        );
    }

    /// Geometry-forced candidate wins even when it is slower on wall-clock.
    #[test]
    fn geometry_forced_overrides_wall_clock() {
        let feed = 3000.0;
        let parallel = parallel_path(feed); // longer / slower on high accel
        let spiral = spiral_path(feed);
        let high = machine_with_accel(5000.0);

        // Mark parallel as geometry-forced even though spiral is faster
        // here — the advisor must still pick parallel.
        let candidates = [
            StrategyCandidate {
                strategy: ClearingStrategy::ContourParallel,
                toolpath: &parallel,
                regime: LoadRegime::ToolLimited,
                geometry_forced: true,
            },
            StrategyCandidate {
                strategy: ClearingStrategy::ContourSpiral,
                toolpath: &spiral,
                regime: LoadRegime::ToolLimited,
                geometry_forced: false,
            },
        ];
        let rec = recommend_strategy(&candidates, &high).unwrap();
        assert_eq!(rec.chosen, ClearingStrategy::ContourParallel);
        assert!(
            rec.reason.contains("geometry forces"),
            "forced pick must explain itself by geometry, got: {}",
            rec.reason
        );
        // It was slower, so no speedup should be claimed.
        assert!(
            !rec.reason.contains("faster than"),
            "a slower forced pick must not claim a speedup, got: {}",
            rec.reason
        );
    }

    /// Empty / degenerate candidates are dropped, not chosen at 0 s.
    #[test]
    fn empty_candidates_are_dropped() {
        let empty = Toolpath::new();
        let parallel = parallel_path(3000.0);
        let machine = machine_with_accel(250.0);
        let candidates = [
            StrategyCandidate {
                strategy: ClearingStrategy::ContourSpiral,
                toolpath: &empty,
                regime: LoadRegime::ToolLimited,
                geometry_forced: false,
            },
            StrategyCandidate {
                strategy: ClearingStrategy::ContourParallel,
                toolpath: &parallel,
                regime: LoadRegime::ToolLimited,
                geometry_forced: false,
            },
        ];
        let rec = recommend_strategy(&candidates, &machine).unwrap();
        assert_eq!(rec.chosen, ClearingStrategy::ContourParallel);
        assert_eq!(rec.ranked.len(), 1, "the empty candidate must be dropped");
    }

    /// No measurable candidates → None.
    #[test]
    fn all_empty_returns_none() {
        let empty = Toolpath::new();
        let machine = machine_with_accel(250.0);
        let candidates = [StrategyCandidate {
            strategy: ClearingStrategy::ContourParallel,
            toolpath: &empty,
            regime: LoadRegime::ToolLimited,
            geometry_forced: false,
        }];
        assert!(recommend_strategy(&candidates, &machine).is_none());
    }
}
