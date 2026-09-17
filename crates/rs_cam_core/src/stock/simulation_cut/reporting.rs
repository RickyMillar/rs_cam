//! Reporting and export: the wall-clock rebase, the cycle-time publisher
//! and the JSON artifact writer with its pruner.

use super::{
    RebasedCuttingTimes, SimulationCutArtifact, SimulationCutSample, SimulationCutTrace,
    ToolpathKinematicRuntime,
};
use crate::ids::ToolpathId;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// **G-AIRDENOM (2026-09-08) — put a toolpath's cutting seconds on the
/// same clock as its `total_runtime_s`.**
///
/// # The defect this closes
///
/// `SummaryAccumulator::observe` accumulates `air_cut_time_s`,
/// `cutting_runtime_s` and `rapid_runtime_s` from `segment_time_s`, which
/// is naive: `segment_len / commanded_feed × 60`
/// (`dexel_stock/simulation.rs`). It carries no acceleration and it uses
/// the PRE-modulation commanded feed. Two later passes then overwrite
/// `total_runtime_s` alone with the kinematics-integrated wall clock
/// (`compute/simulate.rs`, F-034 — accel only; `session/compute.rs`,
/// F-036b — accel plus the MODULATED feed). The result was a ratio whose
/// numerator and denominator came from two different time models: where
/// modulation raised the feed, `cutting_runtime_s` exceeded
/// `total_runtime_s` and [`AirCutRatios::air_cut_pct_of_cutting_time`]
/// read BELOW [`AirCutRatios::air_cut_pct_of_total_runtime`] — the
/// opposite of the documented order. Measured factor 1.76× on a wanaka
/// rough (`planning/ab_instrument_flags_2026-09-08.md` Flag 1).
///
/// # Why the rebase is per SAMPLE, not one factor per toolpath
///
/// The modulator does not move every cutting move by the same factor. A
/// move with no measured engagement short-circuits at its COMMANDED feed
/// (`feed_modulation::adaptive_feed_modulate`, `ConstrainedMax` arm),
/// and an air-cut sample is exactly a sample whose engagement is at or
/// near zero. Scaling the whole toolpath by one average factor would
/// therefore shrink the air seconds along with the engaged seconds and
/// UNDER-report air under a feed raise — the same defect in a smaller
/// coat. Each sample is rebased by its own move's
/// `commanded ÷ modulated` ratio instead.
///
/// The remaining difference between that sum and the integrator's answer
/// is acceleration, which does not correlate with engagement the way
/// modulation does, so it is applied as one uniform factor. The result
/// satisfies `cutting_runtime_s + rapid_runtime_s == breakdown.total_s`
/// exactly, which restores the invariant: the cutting-time percentage is
/// again ≥ the total-runtime percentage.
///
/// # What is NOT rebased
///
/// `average_engagement` stays the time-weighted mean over the COMMANDED
/// clock. It is a comparative signal the repo is calibrated against, and
/// re-weighting it is a separate judgement (CLAUDE.md names it explicitly
/// as relative, not absolute). `KinematicsSummary::cutting_runtime_s`, the
/// `SimulationSemanticCutSummary` rows and `SimulationCutHotspot` keep the
/// naive clock too, so the per-class times no longer sum to the toolpath's.
///
/// The caller DOES move `average_mrr_mm3_s` with the rebase, because it is
/// `total_removed_volume_est_mm3 ÷ cutting_runtime_s` — an identity a
/// reader can check from two published fields.
///
/// # Returns
///
/// `None` when the toolpath has no cutting samples, or when the
/// integrator reports no fed time — leave the summary untouched in both
/// cases rather than writing a zero.
#[must_use]
pub fn rebase_cutting_times(
    samples: &[SimulationCutSample],
    toolpath_id: ToolpathId,
    modulated_feeds: &BTreeMap<(ToolpathId, usize), (f64, crate::tool_load::BindingConstraint)>,
    breakdown: &crate::machine::kinematics::CycleTimeBreakdown,
) -> Option<RebasedCuttingTimes> {
    // The dexel marks a move `is_cutting = false` for `MoveType::Rapid`
    // and for a `Linear` move tagged `MoveIntent::Retract`. The
    // integrator buckets exactly those two sets as `rapid_s` and
    // `retract_s`, so the fed-cutting clock is the remainder.
    let integrated_cutting_s = breakdown.total_s - breakdown.rapid_s - breakdown.retract_s;
    if integrated_cutting_s <= 0.0 {
        return None;
    }

    let mut modulated_sum = 0.0;
    let mut air_sum = 0.0;
    let mut low_sum = 0.0;
    for sample in samples {
        if sample.toolpath_id != toolpath_id || !sample.is_cutting {
            continue;
        }
        let commanded = sample.feed_rate_mm_min;
        if commanded <= 0.0 {
            continue;
        }
        // A move the modulator never visited keeps its commanded feed.
        let achieved = modulated_feeds
            .get(&(toolpath_id, sample.move_index))
            .map_or(commanded, |&(feed, _binding)| feed);
        let scaled = if achieved > 1e-9 {
            sample.segment_time_s * commanded / achieved
        } else {
            sample.segment_time_s
        };
        modulated_sum += scaled;
        // The two thresholds mirror `SummaryAccumulator::observe`; they
        // must stay in step with it or the slices stop partitioning the
        // same population.
        if sample.engagement.radial_woc_fraction < 0.02 {
            air_sum += scaled;
        } else if sample.engagement.radial_woc_fraction < 0.10 {
            low_sum += scaled;
        }
    }
    if modulated_sum <= 1e-12 {
        return None;
    }

    let accel_factor = integrated_cutting_s / modulated_sum;
    Some(RebasedCuttingTimes {
        cutting_runtime_s: integrated_cutting_s,
        air_cut_time_s: air_sum * accel_factor,
        low_engagement_time_s: low_sum * accel_factor,
        rapid_runtime_s: breakdown.rapid_s + breakdown.retract_s,
    })
}

/// Publish a set of kinematics-integrated runtimes onto a trace: the
/// per-toolpath list, the matching engagement summaries, and the project
/// total.
///
/// # Why two callers share one publisher (N2, 2026-09-10)
///
/// The integrator (`crate::compute::simulate::apply_kinematics_cycle_time`)
/// writes this tail after the dexel pass. The adaptive feed modulation
/// re-time (`crate::session::ProjectSession::apply_adaptive_feed_modulation`)
/// writes it again after it rewrites the per-move feeds. Before N2 the
/// re-time carried its own copy of the tail. That copy folded only over
/// `toolpath_summaries`, so a drill — which has no summary row — lost its
/// seconds from the project total, and the copy never rewrote
/// `toolpath_runtimes` at all, so every milling readiness figure stayed at
/// the commanded feeds. One publisher removes both defects together.
///
/// # The project total has two arms, and they are disjoint
///
/// `per_toolpath` is the INTEGRATED set. A summary missing from it was not
/// integrated, so it keeps its own `total_runtime_s` rather than being
/// dropped. The two sets do not overlap by construction: the second arm is
/// exactly the summaries missing from `per_toolpath`.
///
/// # The breakdown fold reads a summary the caller may have written
///
/// The second arm also folds a summary's own `runtime_by_intent` when it
/// carries one. For the integrator that arm adds nothing: every summary
/// reaches this call with `runtime_by_intent: None`, because the summary
/// builders construct the field as `None` and nothing writes it earlier.
/// For the re-time the arm preserves the answer on a project whose machine
/// carries no `kinematics` block. The integrator then publishes no runtimes,
/// the re-time integrates each summary itself, and those breakdowns must
/// still reach the project total.
pub(crate) fn publish_cycle_times(
    trace: &mut SimulationCutTrace,
    per_toolpath: &BTreeMap<ToolpathId, crate::machine::kinematics::CycleTimeBreakdown>,
) {
    // The list is written first and separately because it answers a different
    // question from `toolpath_summaries`: "was this integrated?", not "does
    // this have engagement metrics?". Conflating the two into one slot is what
    // produced G-DRILLTIME.
    trace.toolpath_runtimes = per_toolpath
        .iter()
        .map(|(&toolpath_id, &breakdown)| ToolpathKinematicRuntime {
            toolpath_id,
            breakdown,
        })
        .collect();

    let mut project_breakdown = crate::machine::kinematics::CycleTimeBreakdown::default();
    for tp_summary in &mut trace.toolpath_summaries {
        if let Some(&b) = per_toolpath.get(&tp_summary.toolpath_id) {
            tp_summary.total_runtime_s = b.total_s;
            tp_summary.runtime_by_intent = Some(b);
        }
    }
    let mut project_total = 0.0;
    for b in per_toolpath.values() {
        project_total += b.total_s;
        project_breakdown += *b;
    }
    for tp_summary in &trace.toolpath_summaries {
        if !per_toolpath.contains_key(&tp_summary.toolpath_id) {
            project_total += tp_summary.total_runtime_s;
            if let Some(b) = tp_summary.runtime_by_intent {
                project_breakdown += b;
            }
        }
    }
    trace.summary.total_runtime_s = project_total;
    trace.summary.runtime_by_intent = Some(project_breakdown);
}

/// Write one simulation cut-trace artifact into `dir` and return its path.
///
/// Naming and collision safety live in [`crate::export::artifact_io`]: the name keeps
/// the millisecond stamp as its first `_`-field, which
/// [`prune_simulation_cut_artifacts`] parses for age.
pub fn write_simulation_cut_artifact(
    dir: &Path,
    file_stem: &str,
    artifact: &SimulationCutArtifact,
) -> std::io::Result<PathBuf> {
    crate::export::artifact_io::write_json_artifact(
        dir,
        file_stem,
        "simulation_cut_trace",
        artifact,
    )
}

/// Dumps younger than this are never pruned, whatever the count: concurrent
/// writers (parallel test runs, a second session) share one directory, and a
/// prune racing a write must not delete an artifact its writer is about to
/// read back. The disk-fill scenario this exists for is dumps accumulating
/// over days, which a 10-minute grace does not protect.
const PRUNE_GRACE_MS: u128 = 10 * 60 * 1000;

/// Delete the oldest artifacts in `dir` so at most `keep` remain (G-SIMDUMP).
///
/// Every simulation writes a full cut-trace artifact; at fine resolutions on a
/// large board one dump is multiple GB, and an unbounded directory filled a
/// 935 GB disk (96 GB / 81 dumps observed 2026-08-23). Age is the numeric
/// millisecond-timestamp prefix of the file name written by
/// [`write_simulation_cut_artifact`]; files without one sort oldest. Files
/// younger than [`PRUNE_GRACE_MS`] are always kept. Removal failures are
/// reflected in the returned count of deletions, never as an error — the
/// artifact write itself must not fail because housekeeping did.
pub fn prune_simulation_cut_artifacts(dir: &Path, keep: usize) -> usize {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut dumps: Vec<(u128, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.ends_with(".json") {
                return None;
            }
            let stamp = name
                .split('_')
                .next()
                .and_then(|prefix| prefix.parse::<u128>().ok())
                .unwrap_or(0);
            if now_ms.saturating_sub(stamp) < PRUNE_GRACE_MS {
                return None;
            }
            Some((stamp, path))
        })
        .collect();
    if dumps.len() <= keep {
        return 0;
    }
    dumps.sort_by_key(|(stamp, _)| *stamp);
    let excess = dumps.len() - keep;
    dumps
        .iter()
        .take(excess)
        .filter(|(_, path)| std::fs::remove_file(path).is_ok())
        .count()
}
