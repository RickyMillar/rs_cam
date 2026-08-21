//! The commanded Z ladder, read from the annotated toolpath's span table.
//!
//! # Why this exists
//!
//! adaptive3d publishes its Z ladder as structured spans, so reading the
//! commanded step needs no move-walking, Z-clustering or tolerance heuristic:
//!
//! * `compute::spans::push_adaptive3d_spans` emits one [`SpanKind::DepthPass`]
//!   per `RegionZLevel` / `GlobalZLevel` runtime event carrying
//!   [`SpanPayload::DepthPass`] `{ z_level, pass_index }`. `WaterlineCleanup`
//!   events become [`SpanKind::WaterlineCleanup`] instead and carry **no**
//!   `z_level` — they are excluded here.
//! * Every `SimulationCutSample` carries `span_path: Vec<SpanId>`
//!   (outermost-first), so sample → pass index is a direct join
//!   ([`pass_index_of`]).
//!
//! # MEASURED on the AS013 fixture, 2026-08-21 — read this before building a per-pass ratio bar
//!
//! `ux_3d_terrain.toml` + adaptive3d `depth_per_pass = 3.0`,
//! `stock_to_leave_axial = 0.5`, `fine_stepdown = 0.0`, `z_blend = false`,
//! `RegionOrdering::Global`. The ladder is **20 rungs from 20 DepthPass spans**
//! (19 WaterlineCleanup spans excluded), and it is **not uniform**:
//!
//! ```text
//!   pass  1  z = 54.5652   step = (first rung — no predecessor)
//!   pass  2..19             step = 3.0000   (uniform)
//!   pass 20  z =  0.5000   step = 0.0652    <-- SHORT FINAL PASS
//! ```
//!
//! Two consequences, both of which falsify assumptions made in
//! `planning/perf_review_2026-08-19/RESEARCH_f2_and_aba.md` §A.2:
//!
//! 1. **There is a short final pass.** The last rung lands on the
//!    `stock_to_leave` floor and steps only 0.0652 mm. Re-expressing an
//!    absolute `axial_engagement_mm ≤ dpp + margin` bar as a ratio of the
//!    *local* commanded step therefore does not merely tighten it — it
//!    detonates it. Measured on the F-031 whole-toolpath population: max ratio
//!    **8.6994** (= 0.5673 mm over the 0.0652 mm step, pass 20) against a
//!    proposed 1.35 ceiling, while the unchanged numerator's absolute maximum
//!    is 3.8904 mm, comfortably inside the shipped 4.000 mm bar. A per-pass
//!    ratio is only sound once the short final rung is handled explicitly.
//! 2. **`pass_index` is 1-based here, not 0-based.** The research design said
//!    "pass 0 must abstain"; on this fixture no sample is ever in pass 0 —
//!    measured, 0 of 630 865 — and the rung that actually has no predecessor is
//!    pass **1** (18 764 samples). Any abstention rule keyed on the literal
//!    index 0 is dead code that looks like a safeguard.
//!
//! # The way this goes vacuous
//!
//! `spans_valid == false`: a transform that cannot remap move indices (boundary
//! clip, TSP split) sets it, and an invalidated span table must not be read.
//! [`commanded_z_ladder`] **panics** rather than returning an empty ladder,
//! because an empty ladder makes every sample abstain and any bar built on it
//! reads green over nothing. (Measured: adaptive3d's spans DO survive the AS013
//! dressup / TSP / arc-fit chain with `spans_valid == true`.)

use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, SpanKind, SpanPayload};

/// One rung of the commanded ladder.
#[derive(Debug, Clone, Copy)]
pub struct LadderPass {
    pub pass_index: u32,
    pub z_level: f64,
    /// `z_level[i-1] − z_level[i]`. `None` for the first rung, which has no
    /// predecessor to step from — note that on the AS013 fixture the first rung
    /// is `pass_index == 1`, not 0.
    pub step_mm: Option<f64>,
}

/// The commanded Z ladder plus everything a caller needs to prove it is not
/// reading an empty or ambiguous table.
#[derive(Debug, Clone)]
pub struct Ladder {
    /// One entry per distinct `pass_index`, ascending.
    pub passes: Vec<LadderPass>,
    /// `pass_index` values that appeared with more than one distinct
    /// `z_level`. Non-empty means the ladder is per-region rather than global
    /// (`RegionOrdering::PerRegion` restarts `level_index` inside each region),
    /// so a single step per `pass_index` is ill-defined and callers must not
    /// use [`Self::step_for`].
    pub conflicts: Vec<(u32, Vec<f64>)>,
    /// How many `DepthPass` spans the table carried (before dedup).
    pub depth_pass_spans: usize,
    /// How many `WaterlineCleanup` spans the table carried. Reported so a
    /// reader can see they exist and were excluded, rather than wondering.
    pub waterline_spans: usize,
}

impl Ladder {
    /// The commanded step *into* `pass_index`. `None` for the first rung
    /// (nothing to step from) and for a `pass_index` the ladder does not carry.
    pub fn step_for(&self, pass_index: u32) -> Option<f64> {
        self.passes
            .iter()
            .find(|p| p.pass_index == pass_index)
            .and_then(|p| p.step_mm)
    }

    /// The largest commanded step in the ladder, ignoring the first rung.
    pub fn max_step(&self) -> Option<f64> {
        self.passes
            .iter()
            .filter_map(|p| p.step_mm)
            .fold(None, |acc: Option<f64>, s| {
                Some(acc.map_or(s, |a| a.max(s)))
            })
    }

    /// The smallest commanded step in the ladder, ignoring the first rung.
    pub fn min_step(&self) -> Option<f64> {
        self.passes
            .iter()
            .filter_map(|p| p.step_mm)
            .fold(None, |acc: Option<f64>, s| {
                Some(acc.map_or(s, |a| a.min(s)))
            })
    }

    /// True when every step is the same within `tol`.
    ///
    /// A **short final pass** is what this exists to detect: converting an
    /// absolute `dpp`-based bar into a ratio of the local step is unsound on any
    /// ladder where this returns false. The AS013 fixture returns **false** —
    /// see the module doc.
    pub fn is_uniform(&self, tol: f64) -> bool {
        match (self.min_step(), self.max_step()) {
            (Some(lo), Some(hi)) => (hi - lo).abs() <= tol,
            _ => false,
        }
    }

    /// Human-readable rungs, one line each. Printed by the sentries so the
    /// ladder is on the record and not re-derived by the next reader.
    pub fn describe(&self) -> String {
        let mut out = format!(
            "commanded Z ladder: {} rung(s) from {} DepthPass span(s) \
             ({} WaterlineCleanup span(s) excluded)\n",
            self.passes.len(),
            self.depth_pass_spans,
            self.waterline_spans,
        );
        for p in &self.passes {
            match p.step_mm {
                Some(s) => out.push_str(&format!(
                    "  pass {:>3}  z = {:>9.4}  step = {:>7.4}\n",
                    p.pass_index, p.z_level, s
                )),
                None => out.push_str(&format!(
                    "  pass {:>3}  z = {:>9.4}  step =  (none — first rung, no predecessor)\n",
                    p.pass_index, p.z_level
                )),
            }
        }
        for (idx, zs) in &self.conflicts {
            out.push_str(&format!(
                "  !! pass {idx} carries {} distinct z_levels: {zs:?}\n",
                zs.len()
            ));
        }
        out
    }
}

/// Build the ladder from an annotated toolpath's span table.
///
/// # Panics
///
/// When `spans_valid == false`. See the module doc: an invalidated span table
/// must not be read, and returning an empty ladder instead would make every
/// caller's bar vacuous while looking green.
pub fn commanded_z_ladder(at: &AnnotatedToolpath) -> Ladder {
    assert!(
        at.spans_valid,
        "commanded_z_ladder: `spans_valid` is false — the span table was invalidated by a \
         transform that could not remap move indices (boundary clip / TSP split), so it may \
         not be read. Abstaining loudly rather than returning an empty ladder, which would \
         make every bar built on it vacuous."
    );

    let mut seen: Vec<(u32, Vec<f64>)> = Vec::new();
    let mut depth_pass_spans = 0usize;
    for span in at.spans_of_kind(SpanKind::DepthPass) {
        depth_pass_spans += 1;
        let Some(SpanPayload::DepthPass {
            z_level,
            pass_index,
        }) = span.payload
        else {
            continue;
        };
        match seen.iter_mut().find(|(i, _)| *i == pass_index) {
            Some((_, zs)) => {
                if !zs.iter().any(|z| (z - z_level).abs() <= 1e-9) {
                    zs.push(z_level);
                }
            }
            None => seen.push((pass_index, vec![z_level])),
        }
    }
    let waterline_spans = at.spans_of_kind(SpanKind::WaterlineCleanup).count();

    seen.sort_by_key(|(i, _)| *i);
    let conflicts: Vec<(u32, Vec<f64>)> = seen
        .iter()
        .filter(|(_, zs)| zs.len() > 1)
        .cloned()
        .collect();

    let mut passes: Vec<LadderPass> = Vec::with_capacity(seen.len());
    let mut prev_z: Option<f64> = None;
    for (pass_index, zs) in &seen {
        let z_level = zs[0];
        let step_mm = prev_z.map(|p| p - z_level);
        passes.push(LadderPass {
            pass_index: *pass_index,
            z_level,
            step_mm,
        });
        prev_z = Some(z_level);
    }

    Ladder {
        passes,
        conflicts,
        depth_pass_spans,
        waterline_spans,
    }
}

/// The `pass_index` of the [`SpanKind::DepthPass`] span covering this sample.
///
/// Mirrors `AnnotatedToolpath::classify_span_path`'s rule 4 — walk the path
/// outermost-first, last `DepthPass` on the path wins. Returns `None` when the
/// sample sits under no depth pass (e.g. a `WaterlineCleanup` span, which
/// carries no `z_level`) or when the span table is invalid.
///
/// **The returned index is whatever the generator emitted**, and on AS013 that
/// is 1-based — do not write `if pass_index == 0` expecting it to mean "first
/// pass". Ask [`Ladder::step_for`] instead, which answers `None` for whichever
/// rung is actually first.
pub fn pass_index_of(sample: &SimulationCutSample, at: &AnnotatedToolpath) -> Option<u32> {
    if !at.spans_valid {
        return None;
    }
    let mut found = None;
    for sid in &sample.span_path {
        let Some(span) = at.spans.get(sid.0 as usize) else {
            continue;
        };
        if span.kind == SpanKind::DepthPass
            && let Some(SpanPayload::DepthPass { pass_index, .. }) = span.payload
        {
            found = Some(pass_index);
        }
    }
    found
}
