//! Toolpath intermediate representation.
//!
//! Operations produce Toolpath (not G-code). G-code is a final serialization step.
//! This enables dressups, visualization, and analysis without G-code parsing.

use crate::dropcutter::DropCutterGrid;
use crate::geo::P3;

/// Clearance (mm) above the INPUT STOCK's material ceiling used by the
/// entry-descent post-pass ([`crate::dressup::optimize_entry_descents`]) to
/// split a long safe_z-to-cut-depth plunge into a rapid down to
/// `ceiling + PLUNGE_CLEARANCE_MM` followed by a shorter feed-plunge for the
/// remainder. Clears typical stock_to_leave plus scallop crest.
///
/// Historically (P1 W2) this clearance was measured from a per-XY
/// drop-cutter-sampled MESH height at emission time, inside each generator.
/// That mesh height understates remaining stock on `FromRemainingStock`
/// (rest-machining) ops — the mesh has no notion of "never actually cut
/// here yet" — so a rapid descending to mesh-height + clearance could (and
/// did: 151 rapid collisions on a Rivers rest pass) descend through real
/// material that a prior rough left behind. The ceiling is now always
/// measured against the actual input stock (dexel `max_top_z_in_disc`, or
/// the fresh-stock top when no dexel is available), evaluated once, after
/// generation, as a toolpath post-pass — never inside a generator.
pub const PLUNGE_CLEARANCE_MM: f64 = 2.0;

/// Type of motion for a toolpath move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoveType {
    /// Rapid positioning (G0). Never used for cutting.
    Rapid,
    /// Linear feed move (G1) at specified feed rate (mm/min).
    Linear { feed_rate: f64 },
    /// Clockwise arc (G2) in the XY plane. I/J are offsets from start to center.
    ArcCW { i: f64, j: f64, feed_rate: f64 },
    /// Counter-clockwise arc (G3) in the XY plane. I/J are offsets from start to center.
    ArcCCW { i: f64, j: f64, feed_rate: f64 },
}

/// Per-move classification supplied by the toolpath generator.
///
/// Orthogonal to `MoveType` (kinematic class) and to `ToolpathSemanticTrace`
/// (range-level structural grouping). Lets the simulator distinguish a drill
/// peck from a v-carve plunge entry, and a retract from a feed-through-air,
/// without depending on a kinematic heuristic.
///
/// `Unknown` is a deprecation marker for generators that have not yet
/// migrated. All in-tree generators emit non-`Unknown` tags; the
/// simulator's kinematic classifier remains as the fallback for
/// `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoveIntent {
    /// Drill-cycle plunge (true end-cutting). Engagement metrics do not apply.
    Drilling,
    /// Milling op entering material with a near-pure plunge.
    EntryPlunge,
    /// Roughing material-removal feed.
    ClearingCut,
    /// Finishing-pass feed.
    FinishingCut,
    /// Helical entry into material.
    EntryHelix,
    /// Ramped entry into material.
    EntryRamp,
    /// Tool-position-to-tool-position transition at feed (not material removal).
    Linking,
    /// Lift off material before a rapid (no material removal).
    Retract,
    /// F-040: lead-in arc / tangent entry before the first cut move of a pass.
    /// Carries the dressup's `lead_in_feed_rate` (or the operation feed if
    /// unset). F-039's modulator skips these moves — the operator's lead-in
    /// feed is treated as user-tuned, not modulated.
    LeadIn,
    /// F-040: lead-out arc / tangent exit after the last cut move of a pass.
    LeadOut,
    /// Fallback for legacy / unaware generators. Deprecation marker.
    #[default]
    Unknown,
}

impl MoveType {
    /// True for any cutting move (Linear, ArcCW, ArcCCW). False for Rapid.
    pub fn is_cutting(self) -> bool {
        !matches!(self, MoveType::Rapid)
    }

    /// Feed rate in mm/min, or None for rapids.
    pub fn feed_rate(self) -> Option<f64> {
        match self {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            MoveType::ArcCW { feed_rate, .. } => Some(feed_rate),
            MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
            MoveType::Rapid => None,
        }
    }

    /// Returns a copy of this `MoveType` with the feed rate rewritten to
    /// `feed_rate`, preserving the variant and any arc `i`/`j` offsets.
    /// `Rapid` carries no feed rate and is returned unchanged — callers
    /// don't need to special-case rapids before calling this.
    ///
    /// Canonical helper for feed-rewriting dressups (feed optimization,
    /// adaptive feed modulation) that must not disturb move geometry.
    pub fn with_feed_rate(self, feed_rate: f64) -> MoveType {
        match self {
            MoveType::Rapid => MoveType::Rapid,
            MoveType::Linear { .. } => MoveType::Linear { feed_rate },
            MoveType::ArcCW { i, j, .. } => MoveType::ArcCW { i, j, feed_rate },
            MoveType::ArcCCW { i, j, .. } => MoveType::ArcCCW { i, j, feed_rate },
        }
    }
}

/// A single toolpath move to a target position.
#[derive(Debug, Clone)]
pub struct Move {
    pub target: P3,
    pub move_type: MoveType,
    pub intent: MoveIntent,
}

/// A complete toolpath: a sequence of moves.
#[derive(Debug, Clone, Default)]
pub struct Toolpath {
    pub moves: Vec<Move>,
}

impl Toolpath {
    pub fn new() -> Self {
        Self { moves: Vec::new() }
    }

    pub fn rapid_to(&mut self, target: P3) {
        self.rapid_to_with_intent(target, MoveIntent::Unknown);
    }

    pub fn feed_to(&mut self, target: P3, feed_rate: f64) {
        self.feed_to_with_intent(target, feed_rate, MoveIntent::Unknown);
    }

    pub fn arc_cw_to(&mut self, target: P3, i: f64, j: f64, feed_rate: f64) {
        self.arc_cw_to_with_intent(target, i, j, feed_rate, MoveIntent::Unknown);
    }

    pub fn arc_ccw_to(&mut self, target: P3, i: f64, j: f64, feed_rate: f64) {
        self.arc_ccw_to_with_intent(target, i, j, feed_rate, MoveIntent::Unknown);
    }

    pub fn rapid_to_with_intent(&mut self, target: P3, intent: MoveIntent) {
        self.moves.push(Move {
            target,
            move_type: MoveType::Rapid,
            intent,
        });
    }

    pub fn feed_to_with_intent(&mut self, target: P3, feed_rate: f64, intent: MoveIntent) {
        self.moves.push(Move {
            target,
            move_type: MoveType::Linear { feed_rate },
            intent,
        });
    }

    pub fn arc_cw_to_with_intent(
        &mut self,
        target: P3,
        i: f64,
        j: f64,
        feed_rate: f64,
        intent: MoveIntent,
    ) {
        self.moves.push(Move {
            target,
            move_type: MoveType::ArcCW { i, j, feed_rate },
            intent,
        });
    }

    pub fn arc_ccw_to_with_intent(
        &mut self,
        target: P3,
        i: f64,
        j: f64,
        feed_rate: f64,
        intent: MoveIntent,
    ) {
        self.moves.push(Move {
            target,
            move_type: MoveType::ArcCCW { i, j, feed_rate },
            intent,
        });
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn total_cutting_distance(&self) -> f64 {
        let mut dist = 0.0;
        for i in 1..self.moves.len() {
            match self.moves[i].move_type {
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    let p1 = &self.moves[i - 1].target;
                    let p2 = &self.moves[i].target;
                    dist += (p2 - p1).norm();
                }
                _ => {}
            }
        }
        dist
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Emit rapid→plunge→feed→retract for a 3D path.
    ///
    /// For an empty path, this is a no-op. For a single point, emits
    /// rapid+plunge+retract only (no feed moves).
    ///
    /// Default intents are `Unknown` for all emitted moves. Generators
    /// that know the operation kind should call `emit_path_segment_with_intent`
    /// instead so the simulator can disambiguate plunge entries, cut
    /// bodies, and retracts.
    pub fn emit_path_segment(
        &mut self,
        path: &[P3],
        safe_z: f64,
        feed_rate: f64,
        plunge_rate: f64,
    ) {
        self.emit_path_segment_with_intent(
            path,
            safe_z,
            feed_rate,
            plunge_rate,
            MoveIntent::Unknown,
        );
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Emit rapid(Linking)→plunge(EntryPlunge)→feed(`body_intent`)→rapid(Retract)
    /// for a 3D path.
    ///
    /// `body_intent` tags the feed segments along the path body
    /// (typically `ClearingCut` or `FinishingCut`). The lead-in plunge is
    /// tagged `EntryPlunge` and the lead-out lift is tagged `Retract`,
    /// regardless of `body_intent`.
    ///
    /// Entry-descent optimization (rapid down to just above the input
    /// stock's ceiling before plunging the remainder) is NOT done here —
    /// generators have no view of the actual input stock, only of mesh
    /// surfaces, which understate remaining material on rest-machining
    /// ops. See [`crate::dressup::optimize_entry_descents`], a post-pass
    /// that runs after generation with the real stock in scope.
    pub fn emit_path_segment_with_intent(
        &mut self,
        path: &[P3],
        safe_z: f64,
        feed_rate: f64,
        plunge_rate: f64,
        body_intent: MoveIntent,
    ) {
        if path.is_empty() {
            return;
        }
        self.rapid_to_with_intent(P3::new(path[0].x, path[0].y, safe_z), MoveIntent::Linking);
        self.feed_to_with_intent(path[0], plunge_rate, MoveIntent::EntryPlunge);
        for p in path.iter().skip(1) {
            self.feed_to_with_intent(*p, feed_rate, body_intent);
        }
        if let Some(last) = path.last() {
            self.rapid_to_with_intent(P3::new(last.x, last.y, safe_z), MoveIntent::Retract);
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Emit rapid(Linking)→plunge(EntryPlunge)→feed(`body_intent`)→close→rapid(Retract)
    /// for a closed contour loop (e.g. waterline bands, steep/shallow boundaries).
    ///
    /// `points` are the ordered loop vertices, already carrying Z. The tool
    /// rapids to `safe_z` above `points[0]`, plunges to `points[0]` at
    /// `plunge`, feeds the remaining points at `feed` tagged `body_intent`,
    /// then feeds back to `points[0]` to close the loop — that closing move
    /// is part of the body and also carries `body_intent` — before
    /// retracting to `safe_z`. Loops shorter than 3 points are a no-op (not
    /// enough vertices to form a closed shape).
    ///
    /// Follow-up (tracker `planning/finishing_stack_review_2026-07.md` P1.4):
    /// climb/conventional loop-direction orientation should eventually live
    /// here too, instead of each caller pre-ordering (or not ordering, per
    /// ramp_finish's naive segment-reversal) its own points.
    ///
    /// Entry-descent optimization is NOT done here — see
    /// [`Toolpath::emit_path_segment_with_intent`]'s doc for why that lives
    /// in a post-pass ([`crate::dressup::optimize_entry_descents`]) instead
    /// of inside the emitter.
    pub fn emit_closed_contour_with_intent(
        &mut self,
        points: &[P3],
        safe_z: f64,
        feed: f64,
        plunge: f64,
        body_intent: MoveIntent,
    ) {
        if points.len() < 3 {
            return;
        }
        let first = points[0];
        self.rapid_to_with_intent(P3::new(first.x, first.y, safe_z), MoveIntent::Linking);
        self.feed_to_with_intent(first, plunge, MoveIntent::EntryPlunge);
        for pt in &points[1..] {
            self.feed_to_with_intent(*pt, feed, body_intent);
        }
        // Close the loop — part of the body, tagged the same as the other body feeds.
        self.feed_to_with_intent(first, feed, body_intent);
        self.rapid_to_with_intent(P3::new(first.x, first.y, safe_z), MoveIntent::Retract);
    }

    /// Retract to safe_z if currently below it (0.001mm epsilon).
    pub fn final_retract(&mut self, safe_z: f64) {
        if let Some(last) = self.moves.last()
            && last.target.z < safe_z - 0.001
        {
            self.rapid_to_with_intent(
                P3::new(last.target.x, last.target.y, safe_z),
                MoveIntent::Retract,
            );
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn total_rapid_distance(&self) -> f64 {
        let mut dist = 0.0;
        for i in 1..self.moves.len() {
            if self.moves[i].move_type == MoveType::Rapid {
                let p1 = &self.moves[i - 1].target;
                let p2 = &self.moves[i].target;
                dist += (p2 - p1).norm();
            }
        }
        dist
    }

    /// Sorted unique Z levels in the toolpath, deduplicated with given epsilon.
    pub fn z_levels(&self, epsilon: f64) -> Vec<f64> {
        let mut zs: Vec<f64> = self.moves.iter().map(|m| m.target.z).collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        zs.dedup_by(|a, b| (*a - *b).abs() < epsilon);
        zs
    }

    /// Sorted unique feed rates in the toolpath, deduplicated with given epsilon.
    pub fn feed_rates(&self, epsilon: f64) -> Vec<f64> {
        let mut rates: Vec<f64> = self
            .moves
            .iter()
            .filter_map(|m| match m.move_type {
                MoveType::Linear { feed_rate } => Some(feed_rate),
                MoveType::ArcCW { feed_rate, .. } => Some(feed_rate),
                MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
                MoveType::Rapid => None,
            })
            .collect();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        rates.dedup_by(|a, b| (*a - *b).abs() < epsilon);
        rates
    }

    /// XYZ bounding box of all move targets: ([min_x, min_y, min_z], [max_x, max_y, max_z]).
    /// Returns zeros for empty toolpaths.
    pub fn bounding_box(&self) -> ([f64; 3], [f64; 3]) {
        if self.moves.is_empty() {
            return ([0.0; 3], [0.0; 3]);
        }
        let mut min = [f64::MAX; 3];
        let mut max = [f64::MIN; 3];
        for m in &self.moves {
            let p = &m.target;
            if p.x < min[0] {
                min[0] = p.x;
            }
            if p.y < min[1] {
                min[1] = p.y;
            }
            if p.z < min[2] {
                min[2] = p.z;
            }
            if p.x > max[0] {
                max[0] = p.x;
            }
            if p.y > max[1] {
                max[1] = p.y;
            }
            if p.z > max[2] {
                max[2] = p.z;
            }
        }
        (min, max)
    }
}

/// Simplify a 3D path using Douglas-Peucker with cross-product distance.
///
/// Removes points that deviate less than `tolerance` from the line between
/// their neighbors. Uses 3D perpendicular distance via cross product for
/// accurate distance computation on slopes.
pub fn simplify_path_3d(points: &[P3], tolerance: f64) -> Vec<P3> {
    let keep = simplify_path_3d_keep_mask(points, tolerance);
    points
        .iter()
        .zip(keep.iter())
        .filter(|&(_, &k)| k)
        .map(|(p, _)| *p)
        .collect()
}

/// Douglas-Peucker keep-mask: `result[i] == true` iff point `i` is retained by
/// [`simplify_path_3d`]. Exposed so toolpath-conditioning passes can map the
/// retained points back to their source moves (for feed/intent) rather than
/// re-matching by coordinate.
///
/// Iterative stack-based implementation avoids per-recursion Vec allocations.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn simplify_path_3d_keep_mask(points: &[P3], tolerance: f64) -> Vec<bool> {
    let n = points.len();
    if n <= 2 {
        return vec![true; n];
    }

    // Mark which points to keep. First and last are always kept.
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;

    // Explicit stack of (start, end) index ranges to process.
    let mut stack = Vec::with_capacity(16);
    stack.push((0usize, n - 1));

    while let Some((start, end)) = stack.pop() {
        if end - start < 2 {
            continue;
        }

        let first = points[start];
        let last = points[end];
        let dx = last.x - first.x;
        let dy = last.y - first.y;
        let dz = last.z - first.z;
        let seg_len = (dx * dx + dy * dy + dz * dz).sqrt();

        if seg_len < 1e-10 {
            // Closed-loop case: first ≈ last, so the line first→last is
            // degenerate and RDP's perpendicular-distance metric is
            // undefined. Continuing here would mean the loop's interior
            // points are never even considered for keeping — collapsing
            // any closed perimeter path to just its endpoints (a no-op
            // feed). This was a real bug for callers like the AgentSearch
            // perimeter sweep where the input is a closed offset polygon.
            //
            // Recovery: pick the point furthest from `first` by raw
            // Euclidean distance, keep it, and recurse on each half.
            // Each half then has a non-degenerate baseline so RDP works
            // normally.
            let mut max_eu = 0.0;
            let mut max_idx = start;
            for (i, p) in points
                .iter()
                .enumerate()
                .skip(start + 1)
                .take(end - start - 1)
            {
                let vx = p.x - first.x;
                let vy = p.y - first.y;
                let vz = p.z - first.z;
                let d = (vx * vx + vy * vy + vz * vz).sqrt();
                if d > max_eu {
                    max_eu = d;
                    max_idx = i;
                }
            }
            if max_eu > tolerance {
                keep[max_idx] = true;
                stack.push((start, max_idx));
                stack.push((max_idx, end));
            }
            continue;
        }

        let mut max_dist = 0.0;
        let mut max_idx = start;

        for (i, p) in points
            .iter()
            .enumerate()
            .skip(start + 1)
            .take(end - start - 1)
        {
            let vx = p.x - first.x;
            let vy = p.y - first.y;
            let vz = p.z - first.z;
            let cx = vy * dz - vz * dy;
            let cy = vz * dx - vx * dz;
            let cz = vx * dy - vy * dx;
            let dist = (cx * cx + cy * cy + cz * cz).sqrt() / seg_len;
            if dist > max_dist {
                max_dist = dist;
                max_idx = i;
            }
        }

        if max_dist > tolerance {
            keep[max_idx] = true;
            stack.push((start, max_idx));
            stack.push((max_idx, end));
        }
    }

    keep
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Convert a drop-cutter grid into a zigzag raster toolpath.
///
/// When `min_z` is `Some(z)`, contiguous segments where every point is
/// clamped at `z` (zero engagement) are skipped rather than emitted as
/// flat passes.
pub fn raster_toolpath_from_grid(
    grid: &DropCutterGrid,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    min_z: Option<f64>,
) -> Toolpath {
    let mut tp = Toolpath::new();

    if grid.points.is_empty() {
        return tp;
    }

    /// Epsilon for detecting min_z-clamped points.
    const CLAMP_EPS: f64 = 0.001;

    for row in 0..grid.rows {
        if grid.cols == 0 {
            continue;
        }
        // Zigzag: even rows go left-to-right, odd rows go right-to-left
        let reverse = row % 2 != 0;
        let col_at = |i: usize| -> usize { if reverse { grid.cols - 1 - i } else { i } };

        // When min_z filtering is active, partition the row into segments
        // where at least one point is above the clamp.
        if let Some(clamp_z) = min_z {
            let mut segments: Vec<(usize, usize)> = Vec::new();
            let mut seg_start: Option<usize> = None;
            for i in 0..grid.cols {
                let col = col_at(i);
                let z = grid.get(row, col).z;
                let engaging = z > clamp_z + CLAMP_EPS;
                if engaging {
                    if seg_start.is_none() {
                        seg_start = Some(i);
                    }
                } else if let Some(start) = seg_start.take() {
                    segments.push((start, i - 1));
                }
            }
            if let Some(start) = seg_start {
                segments.push((start, grid.cols - 1));
            }

            // Do NOT merge across gaps: gap points are clamped at min_z, so
            // cutting through them would plunge the tool to the floor. Each
            // contiguous engaging run must be its own pass (retract-traverse-
            // plunge between runs).
            for &(seg_s, seg_e) in &segments {
                let first_col = col_at(seg_s);
                let first_pt = grid.get(row, first_col);
                tp.rapid_to_with_intent(
                    P3::new(first_pt.x, first_pt.y, safe_z),
                    MoveIntent::Linking,
                );
                tp.feed_to_with_intent(first_pt.position(), plunge_rate, MoveIntent::EntryPlunge);

                for i in (seg_s + 1)..=seg_e {
                    let col = col_at(i);
                    let pt = grid.get(row, col);
                    tp.feed_to_with_intent(pt.position(), feed_rate, MoveIntent::FinishingCut);
                }

                let last_col = col_at(seg_e);
                let last_pt = grid.get(row, last_col);
                tp.rapid_to_with_intent(P3::new(last_pt.x, last_pt.y, safe_z), MoveIntent::Retract);
            }
        } else {
            // No min_z filtering — emit the entire row
            let first_col = col_at(0);
            let first_pt = grid.get(row, first_col);
            tp.rapid_to_with_intent(P3::new(first_pt.x, first_pt.y, safe_z), MoveIntent::Linking);
            tp.feed_to_with_intent(first_pt.position(), plunge_rate, MoveIntent::EntryPlunge);

            for i in 1..grid.cols {
                let col = col_at(i);
                let cl = grid.get(row, col);
                tp.feed_to_with_intent(cl.position(), feed_rate, MoveIntent::FinishingCut);
            }

            let last_col = col_at(grid.cols - 1);
            let last_pt = grid.get(row, last_col);
            tp.rapid_to_with_intent(P3::new(last_pt.x, last_pt.y, safe_z), MoveIntent::Retract);
        }
    }

    tp
}

/// Convert a drop-cutter grid into a zigzag raster toolpath with slope filtering.
///
/// Points whose slope angle (from `slope_angles`) falls outside the range
/// `[slope_from, slope_to]` are skipped. Each row is partitioned into
/// contiguous in-range segments. Small gaps (≤ 3 grid steps) between
/// segments are bridged by cutting through them at the surface Z rather
/// than retracting. Retracts happen only between truly distant segments,
/// always at the last cutting point's XY position.
///
/// When `min_z` is `Some(z)`, points clamped at that Z (zero engagement)
/// are also treated as excluded.
///
/// Cutting moves along the row body are tagged `body_intent` (rather than
/// the default `MoveIntent::Unknown` a plain `feed_to` would leave them
/// with), so downstream engagement/air-cut metrics can distinguish this
/// pass's cuts from retracts and other move kinds.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // bounded by grid dimensions
pub fn raster_toolpath_from_grid_with_slope_filter(
    grid: &DropCutterGrid,
    slope_angles: &[f64],
    slope_from: f64,
    slope_to: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    min_z: Option<f64>,
    body_intent: MoveIntent,
) -> Toolpath {
    let mut tp = Toolpath::new();

    if grid.points.is_empty() {
        return tp;
    }

    // Maximum gap (in grid steps) to bridge by cutting through excluded
    // points rather than retracting. Keeps rapids low at slope boundaries
    // where the angle oscillates around the threshold.
    const MAX_BRIDGE_GAP: usize = 3;
    const CLAMP_EPS: f64 = 0.001;

    for row in 0..grid.rows {
        if grid.cols == 0 {
            continue;
        }
        // Zigzag: even rows go left-to-right, odd rows go right-to-left
        let reverse = row % 2 != 0;
        let col_at = |i: usize| -> usize { if reverse { grid.cols - 1 - i } else { i } };

        // Phase 1: Find contiguous in-range segments for this row.
        // A point is "in-range" if its slope is within [slope_from, slope_to]
        // AND it is not clamped at min_z (i.e. it actually engages the surface).
        // Track clamped cells separately — bridging across slope-only gaps is
        // safe (the tool rides the surface through the gap), but bridging
        // across clamped gaps dives the tool to the min_z floor.
        let mut clamped = vec![false; grid.cols];
        let mut in_range = vec![false; grid.cols];
        for (i, (is_clamped, is_in_range)) in
            clamped.iter_mut().zip(in_range.iter_mut()).enumerate()
        {
            let col = col_at(i);
            let idx = row * grid.cols + col;
            let slope_ok = slope_angles
                .get(idx)
                .is_none_or(|&angle| angle >= slope_from && angle <= slope_to);
            let z_ok = min_z.is_none_or(|clamp_z| grid.get(row, col).z > clamp_z + CLAMP_EPS);
            *is_clamped = !z_ok;
            *is_in_range = slope_ok && z_ok;
        }
        let segments = crate::point_runs::split_run_ranges(&in_range, |_, &keep| keep, 1);

        // Phase 2: Merge segments separated by small *slope-only* gaps.
        // Any gap containing a clamped point prevents the merge (cutting
        // through it would plunge to the min_z floor).
        let merged = merge_segments_skip_clamped(&segments, MAX_BRIDGE_GAP, &clamped);

        // Phase 3: Emit toolpath for each merged segment.
        for &(seg_s, seg_e) in &merged {
            let first_col = col_at(seg_s);
            let first_pt = grid.get(row, first_col);

            // Rapid to segment start at safe Z, then plunge
            tp.rapid_to_with_intent(P3::new(first_pt.x, first_pt.y, safe_z), MoveIntent::Linking);
            tp.feed_to_with_intent(first_pt.position(), plunge_rate, MoveIntent::EntryPlunge);

            // Feed through all points in segment (including bridged gaps)
            for i in (seg_s + 1)..=seg_e {
                let col = col_at(i);
                let pt = grid.get(row, col);
                tp.feed_to_with_intent(pt.position(), feed_rate, body_intent);
            }

            // Retract at the last cutting point (vertical, no diagonal)
            let last_col = col_at(seg_e);
            let last_pt = grid.get(row, last_col);
            tp.rapid_to_with_intent(P3::new(last_pt.x, last_pt.y, safe_z), MoveIntent::Retract);
        }
    }

    tp
}

/// Merge segments separated by short gaps, but never merge across a gap that
/// contains any clamped point — cutting through clamped points would dive
/// the tool to the min_z floor.
#[allow(clippy::indexing_slicing)] // first element access guarded by is_empty check
fn merge_segments_skip_clamped(
    segments: &[(usize, usize)],
    max_gap: usize,
    clamped: &[bool],
) -> Vec<(usize, usize)> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut merged = Vec::with_capacity(segments.len());
    let mut current = segments[0];

    for &(start, end) in segments.iter().skip(1) {
        let gap_clean = (current.1 + 1..start).all(|i| !clamped.get(i).copied().unwrap_or(true));
        if start <= current.1 + max_gap + 1 && gap_clean {
            current.1 = end;
        } else {
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);
    merged
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_path_segment_basic() {
        let path = vec![
            P3::new(0.0, 0.0, -1.0),
            P3::new(5.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        ];
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&path, 10.0, 1000.0, 500.0);

        // rapid + plunge + 2 feeds + retract = 5 moves
        assert_eq!(
            tp.moves.len(),
            5,
            "Expected 5 moves, got {}",
            tp.moves.len()
        );
        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert!((tp.moves[0].target.z - 10.0).abs() < 1e-10);
        assert!(
            matches!(tp.moves[1].move_type, MoveType::Linear { feed_rate } if (feed_rate - 500.0).abs() < 1e-10)
        );
        assert!(
            matches!(tp.moves[2].move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
        );
        assert!(
            matches!(tp.moves[3].move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
        );
        assert_eq!(tp.moves[4].move_type, MoveType::Rapid);
        assert!((tp.moves[4].target.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_emit_path_segment_empty() {
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&[], 10.0, 1000.0, 500.0);
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn test_emit_path_segment_single_point() {
        let path = vec![P3::new(5.0, 5.0, -2.0)];
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&path, 10.0, 1000.0, 500.0);

        // rapid + plunge + retract = 3 moves
        assert_eq!(tp.moves.len(), 3);
        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert!(matches!(tp.moves[1].move_type, MoveType::Linear { .. }));
        assert_eq!(tp.moves[2].move_type, MoveType::Rapid);
    }

    #[test]
    fn test_emit_closed_contour_basic() {
        let loop_pts = vec![
            P3::new(0.0, 0.0, -2.0),
            P3::new(10.0, 0.0, -2.0),
            P3::new(10.0, 10.0, -2.0),
            P3::new(0.0, 10.0, -2.0),
        ];
        let mut tp = Toolpath::new();
        tp.emit_closed_contour_with_intent(
            &loop_pts,
            25.0,
            1000.0,
            500.0,
            MoveIntent::FinishingCut,
        );

        // rapid(above first) + plunge(first) + 3 feeds(remaining) + 1 feed(close back
        // to first) + rapid(retract) = 7 moves.
        assert_eq!(tp.moves.len(), 7, "moves: {:?}", tp.moves);

        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert_eq!(tp.moves[0].intent, MoveIntent::Linking);
        assert!((tp.moves[0].target.x - loop_pts[0].x).abs() < 1e-10);
        assert!((tp.moves[0].target.y - loop_pts[0].y).abs() < 1e-10);
        assert!((tp.moves[0].target.z - 25.0).abs() < 1e-10);

        assert!(
            matches!(tp.moves[1].move_type, MoveType::Linear { feed_rate } if (feed_rate - 500.0).abs() < 1e-10)
        );
        assert_eq!(tp.moves[1].intent, MoveIntent::EntryPlunge);
        assert!((tp.moves[1].target.z - loop_pts[0].z).abs() < 1e-10);

        // Body feeds (points[1..]) tagged with body_intent.
        for m in &tp.moves[2..5] {
            assert!(
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
            );
            assert_eq!(m.intent, MoveIntent::FinishingCut);
        }
        assert!((tp.moves[2].target.x - loop_pts[1].x).abs() < 1e-10);
        assert!((tp.moves[3].target.x - loop_pts[2].x).abs() < 1e-10);
        assert!((tp.moves[4].target.x - loop_pts[3].x).abs() < 1e-10);

        // Closing move back to the first point — still body_intent.
        assert!(
            matches!(tp.moves[5].move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
        );
        assert_eq!(tp.moves[5].intent, MoveIntent::FinishingCut);
        assert!((tp.moves[5].target.x - loop_pts[0].x).abs() < 1e-10);
        assert!((tp.moves[5].target.y - loop_pts[0].y).abs() < 1e-10);

        // Retract.
        assert_eq!(tp.moves[6].move_type, MoveType::Rapid);
        assert_eq!(tp.moves[6].intent, MoveIntent::Retract);
        assert!((tp.moves[6].target.z - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_emit_closed_contour_too_few_points_is_noop() {
        let mut tp = Toolpath::new();
        tp.emit_closed_contour_with_intent(
            &[P3::new(0.0, 0.0, -1.0), P3::new(1.0, 0.0, -1.0)],
            10.0,
            1000.0,
            500.0,
            MoveIntent::FinishingCut,
        );
        assert!(tp.moves.is_empty(), "fewer than 3 points must emit nothing");

        let mut tp_empty = Toolpath::new();
        tp_empty.emit_closed_contour_with_intent(&[], 10.0, 1000.0, 500.0, MoveIntent::ClearingCut);
        assert!(tp_empty.moves.is_empty());
    }

    #[test]
    fn test_final_retract_below() {
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(5.0, 5.0, -3.0), 1000.0);
        tp.final_retract(10.0);
        assert_eq!(tp.moves.len(), 2);
        assert_eq!(tp.moves[1].move_type, MoveType::Rapid);
        assert!((tp.moves[1].target.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn emit_path_segment_with_intent_tags_each_phase() {
        let path = vec![
            P3::new(0.0, 0.0, -1.0),
            P3::new(5.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        ];
        let mut tp = Toolpath::new();
        tp.emit_path_segment_with_intent(&path, 10.0, 1000.0, 500.0, MoveIntent::ClearingCut);

        // rapid(Linking) + plunge(EntryPlunge) + feed(ClearingCut) + feed(ClearingCut) + rapid(Retract)
        assert_eq!(tp.moves.len(), 5);
        assert_eq!(tp.moves[0].intent, MoveIntent::Linking);
        assert_eq!(tp.moves[1].intent, MoveIntent::EntryPlunge);
        assert_eq!(tp.moves[2].intent, MoveIntent::ClearingCut);
        assert_eq!(tp.moves[3].intent, MoveIntent::ClearingCut);
        assert_eq!(tp.moves[4].intent, MoveIntent::Retract);
    }

    #[test]
    fn emit_path_segment_default_tags_bookends_leaves_body_unknown() {
        // Non-intent-aware callers (legacy generators) still benefit from
        // the bookend tags — the lead-in plunge is `EntryPlunge` and the
        // lead-out lift is `Retract`. Only the cut body is `Unknown` until
        // the caller is migrated, because we don't know whether it's
        // clearing or finishing without that info. The Retract tag is the
        // load-bearing piece — it makes the simulator's retract-feed
        // reclassification work even for legacy generators.
        let path = vec![
            P3::new(0.0, 0.0, -1.0),
            P3::new(5.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        ];
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&path, 10.0, 1000.0, 500.0);
        assert_eq!(tp.moves[0].intent, MoveIntent::Linking);
        assert_eq!(tp.moves[1].intent, MoveIntent::EntryPlunge);
        // Body moves keep the caller-supplied intent (Unknown for legacy).
        assert_eq!(tp.moves[2].intent, MoveIntent::Unknown);
        assert_eq!(tp.moves[3].intent, MoveIntent::Unknown);
        assert_eq!(tp.moves[4].intent, MoveIntent::Retract);
    }

    #[test]
    fn final_retract_tags_intent() {
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(5.0, 5.0, -3.0), 1000.0);
        tp.final_retract(10.0);
        assert_eq!(tp.moves[1].intent, MoveIntent::Retract);
    }

    #[test]
    fn test_final_retract_already_safe() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(5.0, 5.0, 10.0));
        tp.final_retract(10.0);
        // No retract added since we're already at safe_z
        assert_eq!(tp.moves.len(), 1);
    }

    #[test]
    fn test_simplify_path_3d_collinear() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(2.0, 0.0, 0.0),
            P3::new(3.0, 0.0, 0.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 2, "Collinear points should reduce to 2");
    }

    #[test]
    fn test_simplify_path_3d_preserves_corners() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(5.0, 5.0, 5.0),
            P3::new(10.0, 0.0, 0.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 3, "Corner should be preserved");
    }

    /// Closed-loop input (e.g. a closed perimeter from `parallel_offset`)
    /// must NOT collapse to its endpoints. The original RDP bailed out
    /// when first==last because the perpendicular-distance metric is
    /// undefined for a zero-length baseline; that meant any closed input
    /// became a 2-point no-op feed. This test guards the recovery: the
    /// function picks the furthest-by-Euclidean-distance interior point,
    /// splits the loop, and recurses normally on each half.
    #[test]
    fn test_simplify_path_3d_closed_loop_keeps_corners() {
        // Square traced CCW, returning to start (closed loop).
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
            P3::new(0.0, 0.0, 0.0), // close the loop
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        // Pre-fix this returned 2 points (just first==last). After the fix:
        // the four corners must be kept (plus the closing duplicate). Some
        // corners may be a hair further than others depending on the split
        // point chosen; assert at least 4 (the corners) are preserved.
        assert!(
            simplified.len() >= 4,
            "Closed-loop simplify should keep corners, got {} points: {:?}",
            simplified.len(),
            simplified
        );
        // Specifically: the far corner (10, 10) — the point with maximum
        // Euclidean distance from the start — must be in the kept set.
        let has_far_corner = simplified
            .iter()
            .any(|p| (p.x - 10.0).abs() < 1e-9 && (p.y - 10.0).abs() < 1e-9);
        assert!(
            has_far_corner,
            "Far corner (10,10) must be preserved in closed-loop simplify"
        );
    }

    /// A near-closed loop (start and end nearly coincident, separated by
    /// `< tolerance`) was already handled correctly by RDP — included here
    /// for symmetry with the closed-loop case so future regressions in
    /// either branch are caught.
    #[test]
    fn test_simplify_path_3d_near_closed_keeps_corners() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
            P3::new(0.0, 0.05, 0.0), // 0.05mm gap — RDP works normally
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert!(
            simplified.len() >= 4,
            "Near-closed simplify should keep corners, got {} points",
            simplified.len()
        );
    }

    #[test]
    fn test_z_levels() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(10.0, 0.0, -6.0), 500.0);
        tp.feed_to(P3::new(20.0, 0.0, -6.0), 1000.0);
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));

        let levels = tp.z_levels(0.001);
        assert_eq!(levels, vec![-6.0, -3.0, 10.0]);
    }

    #[test]
    fn test_feed_rates() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));

        let rates = tp.feed_rates(0.1);
        assert_eq!(rates, vec![500.0, 1000.0]);
    }

    #[test]
    fn test_bounding_box() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-5.0, 2.0, 10.0));
        tp.feed_to(P3::new(15.0, -3.0, -8.0), 1000.0);

        let (min, max) = tp.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 1e-10);
        assert!((min[1] - (-3.0)).abs() < 1e-10);
        assert!((min[2] - (-8.0)).abs() < 1e-10);
        assert!((max[0] - 15.0).abs() < 1e-10);
        assert!((max[1] - 2.0).abs() < 1e-10);
        assert!((max[2] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_bounding_box_empty() {
        let tp = Toolpath::new();
        let (min, max) = tp.bounding_box();
        assert_eq!(min, [0.0; 3]);
        assert_eq!(max, [0.0; 3]);
    }

    #[test]
    fn test_toolpath_distances() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(10.0, 0.0, 10.0)); // rapid 10mm
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 100.0); // feed 10mm
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 100.0); // feed 10mm

        assert!((tp.total_rapid_distance() - 10.0).abs() < 1e-10);
        assert!((tp.total_cutting_distance() - 20.0).abs() < 1e-10);
    }

    // --- slope-filtered raster toolpath tests ---

    fn make_test_grid(
        rows: usize,
        cols: usize,
        z_fn: impl Fn(usize, usize) -> f64,
    ) -> DropCutterGrid {
        use crate::tool::CLPoint;
        let mut points = Vec::with_capacity(rows * cols);
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64;
                let y = row as f64;
                let z = z_fn(row, col);
                let mut cl = CLPoint::new(x, y);
                cl.z = z;
                cl.contacted = true;
                points.push(cl);
            }
        }
        DropCutterGrid {
            points,
            rows,
            cols,
            u_start: 0.0,
            v_start: 0.0,
            x_step: 1.0,
            y_step: 1.0,
            direction_deg: 0.0,
        }
    }

    #[test]
    fn test_slope_filter_retract_at_last_cut_position() {
        // Grid: 1 row × 10 cols, flat surface at z=0
        let grid = make_test_grid(1, 10, |_, _| 0.0);
        // Slopes: cols 0-4 are steep (45°), cols 5-9 are flat (0°)
        let mut slopes = vec![0.0; 10];
        for slope in &mut slopes[..5] {
            *slope = 45.0;
        }
        let tp = raster_toolpath_from_grid_with_slope_filter(
            &grid,
            &slopes,
            30.0,
            90.0,
            1000.0,
            500.0,
            10.0,
            None,
            MoveIntent::FinishingCut,
        );
        // Should emit: rapid to (0,0,10) → plunge → feed 1-4 → retract at (4,0,10)
        // The retract should be at x=4 (last in-range col), NOT x=5 (first excluded)
        let retracts: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid && m.target.z > 5.0)
            .collect();
        assert!(!retracts.is_empty());
        let retract = retracts.last().unwrap();
        assert!(
            (retract.target.x - 4.0).abs() < 0.01,
            "Retract should be at x=4 (last cutting col), got x={}",
            retract.target.x
        );
    }

    #[test]
    fn test_slope_filter_bridges_small_gaps() {
        // Grid: 1 row × 20 cols, flat at z=0
        let grid = make_test_grid(1, 20, |_, _| 0.0);
        // Slopes: [steep, steep, flat, steep, steep, ...flat..., steep, steep]
        // Segment 1: cols 0-1 (steep), gap at col 2, segment 2: cols 3-4 (steep)
        // Gap is 1 point → should be bridged
        // Large gap cols 5-16, then segment 3: cols 17-19
        let mut slopes = vec![0.0; 20];
        for &i in &[0, 1, 3, 4, 17, 18, 19] {
            slopes[i] = 60.0;
        }

        let tp = raster_toolpath_from_grid_with_slope_filter(
            &grid,
            &slopes,
            30.0,
            90.0,
            1000.0,
            500.0,
            10.0,
            None,
            MoveIntent::FinishingCut,
        );

        // Count retract-plunge pairs (rapid moves at safe_z that aren't the first)
        let rapids_at_safe: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .collect();

        // With bridging: segments (0-4) and (17-19) → 2 segments
        // = 2 rapids to start + 2 retracts = 4 rapid moves
        // Without bridging: segments (0-1), (3-4), (17-19) → 3 segments = 6 rapid moves
        assert_eq!(
            rapids_at_safe.len(),
            4,
            "Small gap should be bridged, expected 4 rapids (2 segments), got {}",
            rapids_at_safe.len()
        );
    }

    #[test]
    fn test_slope_filter_all_excluded_emits_nothing() {
        let grid = make_test_grid(3, 10, |_, _| 0.0);
        let slopes = vec![0.0; 30]; // all flat
        let tp = raster_toolpath_from_grid_with_slope_filter(
            &grid,
            &slopes,
            30.0,
            90.0,
            1000.0,
            500.0,
            10.0,
            None,
            MoveIntent::FinishingCut,
        );
        assert!(
            tp.moves.is_empty(),
            "All-excluded grid should produce no moves"
        );
    }

    #[test]
    fn test_slope_filter_all_included_matches_unfiltered() {
        let grid = make_test_grid(3, 10, |_, col| col as f64 * 0.5);
        let slopes = vec![45.0; 30]; // all steep
        let tp_filtered = raster_toolpath_from_grid_with_slope_filter(
            &grid,
            &slopes,
            0.0,
            90.0,
            1000.0,
            500.0,
            10.0,
            None,
            MoveIntent::FinishingCut,
        );
        let tp_unfiltered = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0, None);
        assert_eq!(
            tp_filtered.moves.len(),
            tp_unfiltered.moves.len(),
            "All-included slope filter should match unfiltered: {} vs {}",
            tp_filtered.moves.len(),
            tp_unfiltered.moves.len()
        );
    }

    // --- min_z filtering tests ---

    #[test]
    fn test_min_z_reduces_move_count() {
        // Grid: 5 rows × 20 cols. Z values: col 0-4 at z=5 (above clamp),
        // col 5-19 at z=1 (below clamp of z=3).
        let grid = make_test_grid(5, 20, |_, col| if col < 5 { 5.0 } else { 1.0 });

        let tp_no_filter = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0, None);
        let tp_with_filter = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0, Some(3.0));

        assert!(
            tp_with_filter.moves.len() < tp_no_filter.moves.len(),
            "min_z=3 should reduce move count: {} (filtered) vs {} (unfiltered)",
            tp_with_filter.moves.len(),
            tp_no_filter.moves.len()
        );
        // The filtered version should only machine the first ~5 cols per row
        // plus bridged gap points. Much fewer moves than the full 20 cols.
        assert!(
            tp_with_filter.moves.len() < tp_no_filter.moves.len() / 2,
            "Filtered moves ({}) should be less than half of unfiltered ({})",
            tp_with_filter.moves.len(),
            tp_no_filter.moves.len()
        );
    }

    #[test]
    fn test_min_z_all_clamped_emits_nothing() {
        // All points at z=1, min_z=3 → all clamped → no moves
        let grid = make_test_grid(3, 10, |_, _| 1.0);
        let tp = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0, Some(3.0));
        assert!(
            tp.moves.is_empty(),
            "All-clamped grid should produce no moves, got {}",
            tp.moves.len()
        );
    }

    #[test]
    fn test_min_z_none_matches_original() {
        // With min_z=None, should produce same result as before
        let grid = make_test_grid(3, 10, |_, col| col as f64 * 0.5);
        let tp_none = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0, None);
        assert!(
            !tp_none.moves.is_empty(),
            "min_z=None should still produce moves"
        );
        // Every row should have rapids + feeds: 3 rows × (2 rapids + cols feeds)
        assert_eq!(
            tp_none.moves.len(),
            3 * (2 + 10), // 3 rows × (rapid + plunge + 9 feeds + retract)
            "Unfiltered should have all moves"
        );
    }
}
