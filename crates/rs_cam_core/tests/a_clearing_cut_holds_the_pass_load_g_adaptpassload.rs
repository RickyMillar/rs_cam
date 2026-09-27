//! G-ADAPTPASSLOAD — every 2D Adaptive clearing cut holds the commanded
//! stepover as real radial immersion.
//!
//! Operator decision 2026-09-26 (option B,
//! `planning/adaptive_load_2026-09-26/PASSLOAD_PLAN.md`): the load of a 2D
//! Adaptive step is its radial immersion w = sideways material extent / D,
//! the simulator's radial model (physics: `planning/UNIFIED_LOAD_MODEL_
//! 2026-06-18.md` §4, mean force and MRR scale with the extent). One planner
//! predicate, `step_within_pass_load`, holds every producer of a cutting
//! step (agent, gradient, starter pocket, contour loop, boundary loop, mop
//! walk, keep-down link) to the pass ceiling
//! `radial_woc_fraction_from_leading_arc(pass_engagement_limit(s, R))`,
//! 0.3626 at s 2, R 3. Slot-clearing lines are off by default.
//!
//! The oracle is the session simulation of the emitted path (dexel stock,
//! sim cell 0.5 mm): every `ClearingCut` Linear / Arc sample reads a radial
//! of at most the ceiling plus the derived discretisation tolerance,
//! 0.3626 + (0.5 + 0.5 + 0.1) / 6 = 0.546 (`common::adaptive_islands`). The
//! job must still clear the pocket: the simulated removal is at least
//! [`MIN_REMOVED_FRACTION`] of the pocket volume, so a planner that holds
//! the load by leaving stock fails.
//!
//! Red before (a59b246b with the trace markers added and nothing else;
//! `a_clearing_cut_holds_the_pass_load` prints it): 4254 of 8614 samples
//! over 0.546, peak 0.927, median 0.503; per producer (over, peak): residue
//! contour 1868 / 0.927, residue mop 1362 / 0.920, slot line 807 / 0.833,
//! agent 205 / 0.891, starter pocket 12 / 0.834. After: 0 over, peak 0.519.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::collections::BTreeMap;

use common::adaptive_islands::{
    adaptive_session, discretisation_tolerance, pass_limit_radial, toolpath_of, trace_of,
};

use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::{MoveIntent, MoveType};
use rs_cam_core::trace::semantic_trace::ToolpathSemanticKind as Kind;

/// A sample that removes less than this (mm^3) is cutting air.
const CUT_VOLUME_EPS: f64 = 1e-3;

/// The pocket volume, mm^3: (120 x 80 - 6 pi 8^2) x 6 mm depth.
fn pocket_volume_mm3() -> f64 {
    (120.0 * 80.0 - 6.0 * std::f64::consts::PI * 64.0) * 6.0
}

/// The least share of the pocket volume the simulated job must remove. The
/// round cutter leaves the four corner fillets, (1 - pi / 4) R^2 x 6 mm
/// each, 0.09 % of the pocket; the rest of the margin covers the
/// simulator's removal estimate.
const MIN_REMOVED_FRACTION: f64 = 0.97;

/// The fixture an instrument reads: the default (helix entries), or with
/// `RS_CAM_ENTRY_NONE` set, straight-plunge entries.
fn session_for_instrument() -> ProjectSession {
    let style = std::env::var("RS_CAM_ENTRY_NONE")
        .is_ok()
        .then_some(rs_cam_core::compute::config::DressupEntryStyle::None);
    common::adaptive_islands::adaptive_session_with(false, style)
}

/// The producer of move `i`: the adaptive semantic item (one per planner
/// marker) that covers it and opened last, named by its kind and label.
fn producers(session: &ProjectSession) -> Vec<String> {
    let result = session.get_result(0).expect("adaptive result");
    let n = result.toolpath().moves.len();
    let trace = result
        .semantic_trace
        .as_ref()
        .expect("the session attaches a semantic trace");
    let mut best: Vec<(usize, &'static str)> = vec![(0, "(none)"); n];
    for item in &trace.items {
        let (Some(a), Some(b)) = (item.move_start, item.move_end) else {
            continue;
        };
        let name = match item.kind {
            Kind::SlotClearing => "Slot line",
            Kind::Entry if item.label.starts_with("Starter pocket") => "Starter pocket",
            Kind::Entry => "Agent pass",
            Kind::Pass => "After pass",
            Kind::ForcedClear => "Forced clear",
            Kind::Cleanup if item.label.starts_with("Residue mop") => "Residue mop",
            Kind::Cleanup => "Boundary cleanup",
            Kind::Contour if item.label.starts_with("Residue contour") => "Residue contour",
            _ => continue,
        };
        for slot in best.iter_mut().take(b.saturating_add(1)).skip(a) {
            if a >= slot.0 {
                *slot = (a, name);
            }
        }
    }
    best.into_iter().map(|(_, l)| l.to_owned()).collect()
}

#[derive(Debug, Default)]
struct Load {
    samples: usize,
    over: usize,
    /// Samples leaving a straight-plunge entry: after a vertical
    /// `EntryPlunge` (no helix), within R of the plunge point, before the
    /// next entry. The planner exempts those steps (`PassLoad::departing`):
    /// no step out of a hole the cutter's own size reads under half the
    /// diameter. Counted and read here, kept out of `over`.
    exempt: usize,
    exempt_peak: f64,
    peak: f64,
    /// Median over every ClearingCut Linear / Arc sample, air included.
    median: f64,
    /// Median and 90th percentile over the samples that remove stock
    /// (`removed_volume_est_mm3 > CUT_VOLUME_EPS`), and the median weighted
    /// by the volume each sample removes (the radial the bulk is cut at).
    median_cutting: f64,
    p90_cutting: f64,
    median_by_volume: f64,
    /// Median and 90th percentile of each cutting move's peak sample
    /// radial (a move that removes stock). A sample reads the fresh stock
    /// in one short sub-segment's disc, so the samples along a steady side
    /// cut scatter below its width; the move peak is the width it cut.
    median_move_peak: f64,
    p90_move_peak: f64,
    /// Share of the ClearingCut volume by producer.
    volume_share: BTreeMap<String, f64>,
    worst_move: usize,
    /// Per producer: (samples over the bound, peak radial, cutting time s).
    by_producer: BTreeMap<String, (usize, f64, f64)>,
    retracts: usize,
    /// Retracts by the producer that owns the move after them.
    retracts_by_producer: BTreeMap<String, usize>,
    fed_links: usize,
    cycle_s: f64,
    /// Material the simulation removed over the whole job (every sample).
    removed_mm3: f64,
}

/// For each move, the straight-plunge point it may depart from: the XY of
/// the last vertical `EntryPlunge` when no helix entry came after it (the
/// planner exempts the steps inside that plunge's hole).
fn plunge_origin(tp: &rs_cam_core::toolpath::Toolpath) -> Vec<Option<(f64, f64)>> {
    let mut out = Vec::with_capacity(tp.moves.len());
    let mut origin = None;
    let mut prev = None::<rs_cam_core::geo::P3>;
    for m in &tp.moves {
        match m.intent {
            MoveIntent::EntryPlunge
                if prev.is_some_and(|p| {
                    (p.x - m.target.x).hypot(p.y - m.target.y) < 1e-6 && m.target.z < p.z
                }) =>
            {
                origin = Some((m.target.x, m.target.y));
            }
            MoveIntent::EntryHelix => origin = None,
            _ => {}
        }
        out.push(origin);
        prev = Some(m.target);
    }
    out
}

/// `plunge_entries`: the fixture's entries are straight plunges (entry
/// style None), whose departures the planner exempts; else no sample is
/// exempt.
fn measure(session: &ProjectSession, plunge_entries: bool) -> Load {
    let tp = toolpath_of(session);
    let origins = if plunge_entries {
        plunge_origin(&tp)
    } else {
        vec![None; tp.moves.len()]
    };
    let trace = trace_of(session);
    let producer = producers(session);
    let bound = pass_limit_radial() + discretisation_tolerance();
    let mut out = Load {
        cycle_s: trace.summary.total_runtime_s,
        ..Load::default()
    };
    let mut radials = Vec::new();
    let mut cutting: Vec<(f64, f64)> = Vec::new();
    let mut volume: BTreeMap<String, f64> = BTreeMap::new();
    out.removed_mm3 = trace.samples.iter().map(|s| s.removed_volume_est_mm3).sum();
    for s in &trace.samples {
        if s.source_intent != Some(MoveIntent::ClearingCut) {
            continue;
        }
        let Some(mv) = tp.moves.get(s.move_index) else {
            continue;
        };
        if !matches!(
            mv.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        ) {
            continue;
        }
        let r = s.engagement.radial_woc_fraction;
        // Exempt: the move starts inside the plunge hole (the planner exempts
        // a step by where it starts, and keeps the exempt steps a cut of
        // their own so no move mixes the two).
        let start = s.move_index.checked_sub(1).and_then(|j| tp.moves.get(j));
        let departing = origins
            .get(s.move_index)
            .copied()
            .flatten()
            .zip(start)
            .is_some_and(|((x, y), m)| {
                (m.target.x - x).hypot(m.target.y - y)
                    < common::adaptive_islands::TOOL_RADIUS_MM - 1e-9
            });
        if departing {
            out.exempt += 1;
            out.exempt_peak = out.exempt_peak.max(r);
            continue;
        }
        out.samples += 1;
        radials.push(r);
        if s.removed_volume_est_mm3 > CUT_VOLUME_EPS {
            cutting.push((r, s.removed_volume_est_mm3));
            let who = producer
                .get(s.move_index)
                .cloned()
                .unwrap_or_else(|| "(none)".to_owned());
            *volume.entry(who).or_insert(0.0) += s.removed_volume_est_mm3;
        }
        let who = producer
            .get(s.move_index)
            .cloned()
            .unwrap_or_else(|| "(none)".to_owned());
        let entry = out.by_producer.entry(who).or_insert((0, 0.0, 0.0));
        entry.1 = entry.1.max(r);
        entry.2 += s.segment_time_s;
        if r > bound {
            out.over += 1;
            entry.0 += 1;
        }
        if r > out.peak {
            out.peak = r;
            out.worst_move = s.move_index;
        }
    }
    radials.sort_by(f64::total_cmp);
    out.median = radials.get(radials.len() / 2).copied().unwrap_or(0.0);
    let mut move_peak: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    for s in &trace.samples {
        if s.source_intent == Some(MoveIntent::ClearingCut) {
            let e = move_peak.entry(s.move_index).or_insert((0.0, 0.0));
            e.0 = e.0.max(s.engagement.radial_woc_fraction);
            e.1 += s.removed_volume_est_mm3;
        }
    }
    let mut peaks: Vec<f64> = move_peak
        .values()
        .filter(|(_, v)| *v > CUT_VOLUME_EPS)
        .map(|(r, _)| *r)
        .collect();
    peaks.sort_by(f64::total_cmp);
    out.median_move_peak = peaks.get(peaks.len() / 2).copied().unwrap_or(0.0);
    out.p90_move_peak = peaks.get(peaks.len() * 9 / 10).copied().unwrap_or(0.0);
    cutting.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.median_cutting = cutting.get(cutting.len() / 2).map_or(0.0, |c| c.0);
    out.p90_cutting = cutting.get(cutting.len() * 9 / 10).map_or(0.0, |c| c.0);
    let total: f64 = cutting.iter().map(|c| c.1).sum();
    let mut acc = 0.0;
    for &(r, v) in &cutting {
        acc += v;
        if acc >= total / 2.0 {
            out.median_by_volume = r;
            break;
        }
    }
    out.volume_share = volume
        .into_iter()
        .map(|(k, v)| (k, v / total.max(1e-9)))
        .collect();
    for (i, w) in tp.moves.windows(2).enumerate() {
        if matches!(w[1].move_type, MoveType::Rapid) && w[1].target.z > w[0].target.z {
            out.retracts += 1;
            let who = producer
                .get(i + 1)
                .cloned()
                .unwrap_or_else(|| "(none)".to_owned());
            *out.retracts_by_producer.entry(who).or_insert(0) += 1;
        }
    }
    out.fed_links = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(i, m)| {
            *i > 0 && m.intent == MoveIntent::Linking && !matches!(m.move_type, MoveType::Rapid)
        })
        .count();
    out
}

/// Every `ClearingCut` sample the simulator reads stays within the pass
/// load plus the discretisation tolerance.
#[test]
fn a_clearing_cut_holds_the_pass_load() {
    let session = adaptive_session(false);
    let load = measure(&session, false);
    let limit = pass_limit_radial();
    let tol = discretisation_tolerance();
    eprintln!(
        "G-ADAPTPASSLOAD: limit {limit:.4} + tol {tol:.4}, cycle {:.1} s: {load:#?}",
        load.cycle_s
    );
    assert!(
        load.samples > 1000,
        "only {} ClearingCut samples: the fixture tests nothing",
        load.samples
    );
    let walls = wall_cuts(&session);
    assert!(
        walls.is_empty(),
        "moves cut a wall of the part (count, deepest mm by intent): {walls:?}"
    );
    assert!(
        load.removed_mm3 >= MIN_REMOVED_FRACTION * pocket_volume_mm3(),
        "the job removed {:.0} mm^3 of a {:.0} mm^3 pocket: holding the load by leaving stock",
        load.removed_mm3,
        pocket_volume_mm3()
    );
    assert_eq!(
        load.over,
        0,
        "{} ClearingCut samples read a radial above {:.4} (peak {:.4} on move {}); per \
         producer (over, peak): {:?}",
        load.over,
        limit + tol,
        load.peak,
        load.worst_move,
        load.by_producer
    );
}

/// Instrument: each over-bound sample with the move before its run of
/// cutting moves (what the cutter was doing just before).
#[test]
#[ignore = "instrument: prints each ClearingCut sample over the bound"]
fn print_each_sample_over_the_bound() {
    let session = session_for_instrument();
    let tp = toolpath_of(&session);
    let trace = trace_of(&session);
    let producer = producers(&session);
    let bound = pass_limit_radial() + discretisation_tolerance();
    let mut last_move = usize::MAX;
    for s in &trace.samples {
        if s.source_intent != Some(MoveIntent::ClearingCut)
            || s.engagement.radial_woc_fraction <= bound
            || s.move_index == last_move
        {
            continue;
        }
        last_move = s.move_index;
        let mut k = s.move_index;
        while k > 0 && tp.moves[k - 1].intent == MoveIntent::ClearingCut {
            k -= 1;
        }
        let before = k.checked_sub(1).map(|j| tp.moves[j].intent);
        let from = tp.moves[s.move_index - 1].target;
        let to = tp.moves[s.move_index].target;
        eprintln!(
            "move {:5} ({:>15}) radial {:.3} run start {} ({} moves in) after {:?}; ({:.2},{:.2})->({:.2},{:.2}) z {:.1}",
            s.move_index,
            producer[s.move_index],
            s.engagement.radial_woc_fraction,
            k,
            s.move_index - k,
            before,
            from.x,
            from.y,
            to.x,
            to.y,
            to.z
        );
    }
}

/// Instrument: the moves around `RS_CAM_MOVE` with their peak radial.
#[test]
#[ignore = "instrument: prints the moves around one move index"]
fn print_moves_around() {
    let at: usize = std::env::var("RS_CAM_MOVE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(890);
    let session = session_for_instrument();
    let tp = toolpath_of(&session);
    let trace = trace_of(&session);
    for i in at.saturating_sub(12)..(at + 3).min(tp.moves.len()) {
        let r = trace
            .samples
            .iter()
            .filter(|s| s.move_index == i)
            .map(|s| s.engagement.radial_woc_fraction)
            .fold(0.0_f64, f64::max);
        let m = &tp.moves[i];
        eprintln!(
            "{i:5} {:?} {:?} ({:.3},{:.3},{:.2}) radial {r:.3}",
            m.intent, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }
}

/// Every move below the stock top whose cutter crosses a wall of the part
/// (the pocket polygon, islands included): count and deepest cut, by
/// intent. Measured against the polygon the planner and the dressup read.
fn wall_cuts(session: &ProjectSession) -> BTreeMap<String, (usize, f64)> {
    let tp = toolpath_of(session);
    let r = common::adaptive_islands::TOOL_RADIUS_MM;
    let wall = common::adaptive_islands::wall_distance;
    let mut by_intent: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    let mut prev = None::<rs_cam_core::geo::P3>;
    for m in &tp.moves {
        let from = prev.unwrap_or(m.target);
        prev = Some(m.target);
        if m.target.z > -0.01 || matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        // Sample the move every 0.1 mm, along its arc for G2 / G3.
        let arc = match m.move_type {
            MoveType::ArcCW { i, j, .. } => Some((i, j, true)),
            MoveType::ArcCCW { i, j, .. } => Some((i, j, false)),
            _ => None,
        };
        let point_at = |t: f64| -> (f64, f64) {
            match arc {
                Some((i, j, cw)) => {
                    let (cx, cy) = (from.x + i, from.y + j);
                    let rad = i.hypot(j);
                    let a0 = (from.y - cy).atan2(from.x - cx);
                    let a1 = (m.target.y - cy).atan2(m.target.x - cx);
                    let tau = std::f64::consts::TAU;
                    let sweep = if cw {
                        -((a0 - a1).rem_euclid(tau))
                    } else {
                        (a1 - a0).rem_euclid(tau)
                    };
                    let a = a0 + t * sweep;
                    (cx + rad * a.cos(), cy + rad * a.sin())
                }
                None => (
                    from.x + t * (m.target.x - from.x),
                    from.y + t * (m.target.y - from.y),
                ),
            }
        };
        let len = (m.target.x - from.x).hypot(m.target.y - from.y) * 2.0;
        let n = ((len / 0.1).ceil() as usize).max(1);
        let mut depth = 0.0_f64;
        for k in 0..=n {
            let (x, y) = point_at(k as f64 / n as f64);
            depth = depth.max(r - wall(x, y));
        }
        if depth > WALL_CUT_EPS_MM {
            let e = by_intent
                .entry(format!("{:?}", m.intent))
                .or_insert((0, 0.0));
            e.0 += 1;
            e.1 = e.1.max(depth);
        }
    }
    by_intent
}

/// A wall "cut" at or below this is the flattening of the tool-centre
/// region, not a cut: the planner reads the part inset by R with its arcs
/// flattened once at `FlattenPolicy::UNTOLERANCED_MM` (points on the true
/// arc, so a chord sits inside the inset by at most that). Measured on
/// 2026-09-26 (round 3): deepest 0.0063 mm (EntryHelix), ClearingCut
/// 0.0047, Linking 0.0011. Before the region was flattened this way and
/// before a move was read as a segment, ClearingCut 0.081 mm and Linking
/// (an arc fit bulging into the wall) 0.041 mm.
const WALL_CUT_EPS_MM: f64 = rs_cam_core::polygon::FlattenPolicy::UNTOLERANCED_MM;

/// Instrument: [`wall_cuts`] printed.
#[test]
#[ignore = "instrument: counts moves that cut a wall"]
fn print_entry_wall_gouges() {
    let session = session_for_instrument();
    eprintln!(
        "moves below the top whose cutter crosses a wall (count, worst mm): {:?}",
        wall_cuts(&session)
    );
}

/// Instrument: each agent pass summary (steps, exit reason).
#[test]
#[ignore = "instrument: prints the agent pass summaries"]
fn print_agent_pass_summaries() {
    use rs_cam_core::trace::semantic_trace::SemanticKey;
    let session = adaptive_session(false);
    let result = session.get_result(0).expect("adaptive result");
    let trace = result.semantic_trace.as_ref().expect("semantic trace");
    let mut reasons: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for item in trace.items.iter().filter(|i| i.kind == Kind::Pass) {
        let reason = item
            .params
            .get(SemanticKey::ExitReason)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
        let steps = item
            .params
            .get(SemanticKey::StepCount)
            .and_then(|v| v.as_u64())
            .unwrap_or_default() as usize;
        let e = reasons.entry(reason).or_insert((0, 0));
        e.0 += 1;
        e.1 += steps;
    }
    eprintln!("agent passes by exit reason (passes, steps): {reasons:?}");
}

/// Instrument: replay the emitted moves at the first level up to
/// `RS_CAM_MOVE` on a 0.5 mm lattice (the planner's) and print the window
/// `RS_CAM_WIN` = "x0,y0" (12 x 12 mm): '#' stock the emitted path has not
/// cut, '.' cut. Compare with the planner's own grid.
#[test]
#[ignore = "instrument: prints the emitted-path stock around a point"]
fn print_emitted_stock_window() {
    let upto: usize = std::env::var("RS_CAM_MOVE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1113);
    let (x0, y0) = std::env::var("RS_CAM_WIN")
        .ok()
        .and_then(|v| {
            let mut it = v.split(',').map(|t| t.parse::<f64>().ok());
            Some((it.next()??, it.next()??))
        })
        .unwrap_or((-23.0, 12.0));
    let session = session_for_instrument();
    let tp = toolpath_of(&session);
    let r = common::adaptive_islands::TOOL_RADIUS_MM;
    let pitch: f64 = std::env::var("RS_CAM_PITCH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.5);
    let n = (12.0 / pitch) as usize + 1;
    let mut cut = vec![false; n * n];
    let mut prev = tp.moves[0].target;
    for m in tp.moves.iter().take(upto + 1) {
        let to = m.target;
        if to.z < -2.9 && prev.z < -2.9 && to.z > -3.1 {
            let len = (to.x - prev.x).hypot(to.y - prev.y);
            let k = ((len / 0.05).ceil() as usize).max(1);
            for s in 0..=k {
                let t = s as f64 / k as f64;
                let (px, py) = (prev.x + t * (to.x - prev.x), prev.y + t * (to.y - prev.y));
                for j in 0..n {
                    for i in 0..n {
                        let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
                        if (x - px).hypot(y - py) <= r {
                            cut[j * n + i] = true;
                        }
                    }
                }
            }
        }
        prev = to;
    }
    let mut rows = String::new();
    for j in (0..n).rev() {
        rows.push_str(&format!("\n{:5.1} ", y0 + j as f64 * pitch));
        for i in 0..n {
            rows.push(if cut[j * n + i] { '.' } else { '#' });
        }
    }
    eprintln!("emitted stock up to move {upto} (arcs as chords):{rows}");
}

/// Instrument: every sim sample of moves `RS_CAM_MOVE` - 1 ..= + 1.
#[test]
#[ignore = "instrument: prints the sim samples around one move"]
fn print_samples_around() {
    let at: usize = std::env::var("RS_CAM_MOVE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1114);
    let session = session_for_instrument();
    let trace = trace_of(&session);
    for s in trace
        .samples
        .iter()
        .filter(|s| s.move_index + 1 >= at && s.move_index <= at + 1)
    {
        eprintln!(
            "move {} sample {} at ({:.2},{:.2},{:.2}) radial {:.3} removed {:.3} axial {:.2}",
            s.move_index,
            s.sample_index,
            s.position[0],
            s.position[1],
            s.position[2],
            s.engagement.radial_woc_fraction,
            s.removed_volume_est_mm3,
            s.axial_engagement_mm
        );
    }
}

/// Instrument: the simulated time by move intent, and by producer for the
/// non-cutting intents.
#[test]
#[ignore = "instrument: prints the cycle time by intent"]
fn print_time_by_intent() {
    let session = session_for_instrument();
    let trace = trace_of(&session);
    let producer = producers(&session);
    let mut by: BTreeMap<String, f64> = BTreeMap::new();
    for s in &trace.samples {
        let who = producer.get(s.move_index).cloned().unwrap_or_default();
        *by.entry(format!("{:?} / {who}", s.source_intent))
            .or_insert(0.0) += s.segment_time_s;
    }
    eprintln!(
        "time by intent / producer: {by:#?}; total {:.1}",
        trace.summary.total_runtime_s
    );
}

/// Sentry: the straight-plunge exemption, made visible and held. With the
/// entry style set to None (a vertical plunge at every entry) the samples
/// that leave a plunge hole are counted and read (`exempt`,
/// `exempt_peak`); every other ClearingCut sample is held to the bound,
/// and no move cuts a wall. Before round 3 (2026-09-26) 14 non-exempt
/// samples read over (peak 0.92): steps across slivers the centre lattice
/// could not see (two either side of a disc), and slivers the emitted
/// chord left where the planner had stamped its walked steps. The planner
/// now reads the slivers on sub-points (`compute_swept_width_with_slivers`)
/// and emits the path it walked.
#[test]
fn a_straight_plunge_entry_is_exempt_only_leaving_its_hole() {
    let session = common::adaptive_islands::adaptive_session_with(
        false,
        Some(rs_cam_core::compute::config::DressupEntryStyle::None),
    );
    let load = measure(&session, true);
    let bound = pass_limit_radial() + discretisation_tolerance();
    eprintln!(
        "G-ADAPTPASSLOAD plunge entries: exempt {} (peak {:.4}), over {} of {} (peak {:.4}), cycle {:.1} s",
        load.exempt, load.exempt_peak, load.over, load.samples, load.peak, load.cycle_s
    );
    assert_eq!(
        load.over, 0,
        "{} non-exempt ClearingCut samples over {bound:.4} (peak {:.4} on move {}): {:?}",
        load.over, load.peak, load.worst_move, load.by_producer
    );
    assert!(load.samples > 1000, "too few samples: {}", load.samples);
    let walls = wall_cuts(&session);
    assert!(walls.is_empty(), "moves cut a wall of the part: {walls:?}");
}

/// The points along move `mv` from `from`, every `pitch` / 2 mm (arcs
/// followed).
fn move_points(
    from: rs_cam_core::geo::P3,
    mv: &rs_cam_core::toolpath::Move,
    pitch: f64,
) -> Vec<(f64, f64)> {
    let arc = match mv.move_type {
        MoveType::ArcCW { i, j, .. } => Some((i, j, true)),
        MoveType::ArcCCW { i, j, .. } => Some((i, j, false)),
        _ => None,
    };
    let len = (mv.target.x - from.x).hypot(mv.target.y - from.y) * 2.0;
    let k = ((len / pitch).ceil() as usize).max(1);
    (0..=k)
        .map(|s| {
            let t = s as f64 / k as f64;
            match arc {
                Some((i, j, cw)) => {
                    let (cx, cy) = (from.x + i, from.y + j);
                    let rad = i.hypot(j);
                    let a0 = (from.y - cy).atan2(from.x - cx);
                    let a1 = (mv.target.y - cy).atan2(mv.target.x - cx);
                    let tau = std::f64::consts::TAU;
                    let sweep = if cw {
                        -((a0 - a1).rem_euclid(tau))
                    } else {
                        (a1 - a0).rem_euclid(tau)
                    };
                    let a = a0 + t * sweep;
                    (cx + rad * a.cos(), cy + rad * a.sin())
                }
                None => (
                    from.x + t * (mv.target.x - from.x),
                    from.y + t * (mv.target.y - from.y),
                ),
            }
        })
        .collect()
}

/// Lattice pitch (mm) of the exact geometry reading: a tenth of the
/// simulation's cell (`SIM_RESOLUTION_MM`), the reading's own
/// discretisation then a tenth of the one the load bar already carries.
const EXACT_PITCH_MM: f64 = common::adaptive_islands::SIM_RESOLUTION_MM / 10.0;

/// The swept width of the sub-segment `from → to` of move `m`, read the way
/// the simulation reads a sample (the stock standing in the disc at the
/// sub-segment's midpoint, sideways extent over D) but on the exact
/// geometry: a lattice at [`EXACT_PITCH_MM`] around the sample, cut by
/// every earlier move at or below the move's level as the capsule the
/// cutter sweeps (arcs followed; a descent cuts where it lands), then by
/// move `m` up to `from`. The simulation blends a partly covered dexel
/// (`ray_blend_above` keeps `1 - coverage` of its height), so a cell two
/// stamps cover in parts keeps `(1 - f1)(1 - f2)` of its height where the
/// union of the two discs may cover all of it; this reading has no blend.
fn exact_width_at(
    tp: &rs_cam_core::toolpath::Toolpath,
    m: usize,
    from: (f64, f64),
    to: (f64, f64),
) -> f64 {
    let r = common::adaptive_islands::TOOL_RADIUS_MM;
    let pitch = EXACT_PITCH_MM;
    let z = tp.moves[m].target.z;
    let half = 2.0 * r;
    let n = (2.0 * half / pitch) as usize + 1;
    let (x0, y0) = (to.0 - half, to.1 - half);
    let mut cut = vec![false; n * n];
    let stamp = |cut: &mut Vec<bool>, px: f64, py: f64| {
        if (px - to.0).abs() > half + r || (py - to.1).abs() > half + r {
            return;
        }
        let i0 = ((px - r - x0) / pitch).floor().max(0.0) as usize;
        let i1 = (((px + r - x0) / pitch).ceil().max(0.0) as usize).min(n - 1);
        let j0 = ((py - r - y0) / pitch).floor().max(0.0) as usize;
        let j1 = (((py + r - y0) / pitch).ceil().max(0.0) as usize).min(n - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
                if (x - px).hypot(y - py) <= r {
                    cut[j * n + i] = true;
                }
            }
        }
    };
    let mut prev = tp.moves[0].target;
    for mv in tp.moves.iter().take(m) {
        if mv.target.z <= z + 0.01 && prev.z <= z + 0.01 {
            for (px, py) in move_points(prev, mv, pitch) {
                stamp(&mut cut, px, py);
            }
        } else if mv.target.z <= z + 0.01 {
            stamp(&mut cut, mv.target.x, mv.target.y);
        }
        prev = mv.target;
    }
    for (px, py) in move_points(prev, &tp.moves[m], pitch) {
        stamp(&mut cut, px, py);
        if (px - from.0).hypot(py - from.1) < pitch {
            break;
        }
    }
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l = dx.hypot(dy).max(1e-9);
    let (sx, sy) = (-dy / l, dx / l);
    let mid = (0.5 * (from.0 + to.0), 0.5 * (from.1 + to.1));
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
            if !cut[j * n + i] && (x - mid.0).hypot(y - mid.1) <= r {
                let u = (x - mid.0) * sx + (y - mid.1) * sy;
                lo = lo.min(u);
                hi = hi.max(u);
            }
        }
    }
    if hi > lo { (hi - lo) / (2.0 * r) } else { 0.0 }
}

/// Instrument: the exact swept width ([`exact_width_at`]) at each sample
/// of move `RS_CAM_MOVE`, beside the simulation's reading.
#[test]
#[ignore = "instrument: exact swept width along one move"]
fn print_exact_width_along_move() {
    let m: usize = std::env::var("RS_CAM_MOVE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2573);
    let session = session_for_instrument();
    let tp = toolpath_of(&session);
    let trace = trace_of(&session);
    let mut last = (tp.moves[m - 1].target.x, tp.moves[m - 1].target.y);
    for s in trace.samples.iter().filter(|s| s.move_index == m) {
        let at = (s.position[0], s.position[1]);
        let exact = exact_width_at(&tp, m, last, at);
        eprintln!(
            "sample at ({:.2},{:.2}): simulation {:.3}, exact {exact:.3}",
            at.0, at.1, s.engagement.radial_woc_fraction
        );
        last = at;
    }
}

/// Instrument: the helix entries (each run of `EntryHelix` moves): count,
/// arcs and lines, the median and least arc radius, and the time per
/// entry (the overhead lever list in the plan's round-3 results).
#[test]
#[ignore = "instrument: prints the helix entry statistics"]
fn print_entry_stats() {
    let session = session_for_instrument();
    let tp = toolpath_of(&session);
    let trace = trace_of(&session);
    let mut time_of: BTreeMap<usize, f64> = BTreeMap::new();
    for s in &trace.samples {
        *time_of.entry(s.move_index).or_insert(0.0) += s.segment_time_s;
    }
    let (mut arcs, mut lines) = (0usize, 0usize);
    let mut radii = Vec::new();
    let mut times = Vec::new();
    let mut current: Option<f64> = None;
    // Per entry: its largest arc radius (0 for an all-line helix), time and
    // descent (mm).
    let mut entry: Vec<(f64, f64, f64)> = Vec::new();
    let mut cur_r = 0.0_f64;
    let mut cur_top = f64::NEG_INFINITY;
    let mut cur_low = f64::INFINITY;
    for (i, m) in tp.moves.iter().enumerate() {
        if m.intent == MoveIntent::EntryHelix {
            if current.is_none() {
                cur_top = i
                    .checked_sub(1)
                    .and_then(|k| tp.moves.get(k))
                    .map_or(m.target.z, |p| p.target.z);
            }
            cur_low = cur_low.min(m.target.z);
            *current.get_or_insert(0.0) += time_of.get(&i).copied().unwrap_or(0.0);
            match m.move_type {
                MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                    arcs += 1;
                    radii.push(i.hypot(j));
                    cur_r = cur_r.max(i.hypot(j));
                }
                _ => lines += 1,
            }
        } else if let Some(t) = current.take() {
            times.push(t);
            entry.push((cur_r, t, cur_top - cur_low));
            cur_r = 0.0;
            cur_low = f64::INFINITY;
        }
    }
    if let Some(t) = current {
        times.push(t);
        entry.push((cur_r, t, cur_top - cur_low));
    }
    // By arc radius band: entries, time, mean descent.
    let mut bands: BTreeMap<String, (usize, f64, f64)> = BTreeMap::new();
    for &(r, t, dz) in &entry {
        let band = match r {
            r if r < 0.5 => "a: r < 0.5",
            r if r < 1.0 => "b: 0.5 <= r < 1.0",
            r if r < 1.7 => "c: 1.0 <= r < 1.7",
            _ => "d: r >= 1.7",
        };
        let e = bands.entry(band.to_owned()).or_insert((0, 0.0, 0.0));
        e.0 += 1;
        e.1 += t;
        e.2 += dz;
    }
    eprintln!("helix entries by largest arc radius (entries, time s, total descent mm): {bands:?}");
    radii.sort_by(f64::total_cmp);
    times.sort_by(f64::total_cmp);
    let mid = |v: &[f64]| v.get(v.len() / 2).copied().unwrap_or(0.0);
    eprintln!(
        "helix entries {}: arcs {arcs}, lines {lines}, arc radius median {:.3} least {:.3}; \
         time per entry median {:.2} s, most {:.2} s, total {:.1} s",
        times.len(),
        mid(&radii),
        radii.first().copied().unwrap_or(0.0),
        mid(&times),
        times.last().copied().unwrap_or(0.0),
        times.iter().sum::<f64>()
    );
}
