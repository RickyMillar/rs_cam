# P2 C10 — why the C8 lead-in never applied, and where a growing bite fits (2026-10-24)

Status: READ-ONLY analysis. No cargo. No source edit. The patch below is
proposed and not applied. Base: `adaptive-instruments` at 609ce75b. All
`file:line` citations are at 609ce75b (`git show 609ce75b:<path>`). C9
edits `research_geodesic.rs` in this worktree now, so the working-file
line numbers are about 174 lines later.

Every number comes from R 3, s 2, the cell 0.5 (`planner_cell_size`, R/6),
the tolerance 0.1 and c·D 2.1756. A number from the Python model is a
PREDICTION until the verifier reads it in the harness.

## Summary

- **Q1: two causes. One is a limit and one is a bug. The "0.0 to 0.0" is
  a third, a gap in the readout.**
  1. Readout gap. The radius range is written only on success
     (`research_geodesic.rs:2887-2893` for bands, `:3153-3159` for closed
     wraps). So "0.0 to 0.0" means only "0 successes". It does not show
     where the rejection happens.
  2. LIMIT, inside ρ\* (22.78 mm). The curvature law spends all the
     margin there, so `m ≈ 0` (±1e-5 from float noise). Then
     `lead_in_radius` returns `None` at `:2701` (m ≤ 0), or gives
     `ρ_min` of 10⁴–10¹¹ mm, so `ρ_L → ρ` and the guard
     `radius > rho - delta/2` at `:2758` rejects it. Every closed wrap
     after the core is in this zone. So are all pocket lobe bands and
     the inner star bands (model: 15 of 39 star bands, 12 of 12 pocket
     bands).
  3. BUG plus a LIMIT, beyond ρ\*. `lead_band` traces the lead-in at
     mid-arc first and returns on a miss with `?` (`:2810`). A trace
     misses whenever the span is more than half the arc, because the
     lead circle walks off the end of the cut arc `a`. Then no point of
     `a` comes within one cell, and the right-angle guard at `:2771-2773`
     fails. So the planned move of P "further from E" (`:2814`) can never
     run in the case it exists for, and the `at >= total` test (`:2815`)
     is almost never reached. On the star this hides a real limit: every
     arm arc beyond ρ\* is shorter than the span (10.6 mm against 20+ mm
     at level 11, 4.2 mm against 16.2 mm at level 15). On the long pocket
     arcs it is the bug alone.
- **Fix.** Scan P from mid-arc toward F, one cell at a time. Take the
  first P whose traced start lies on `a`. Also count each miss by reason
  (patch below). Model prediction: star lobe bands 0 → 0. Pocket lobe
  bands 0 → 0. Pocket, over all open arcs with a parent: 1 → 10 of 158
  arc ends. The fix does not rescue C8 on the star.
- **Q2: the growing bite.** A bite that grows along the arc under the
  `EntryLaw` tilt bound, with the circular excess, is 1.4–2.3 times
  shorter than the circular lead-in (for example 11.6 mm against 19.4 mm
  at ρ 30). It still fits **0 of 39 star lobe bands** and 0 of 98 star
  arc ends at spacing s. **It cannot reduce the 58 star wrap-end joins
  at the current spacing.** It fits all star arm bands from level 8 to
  15 if the band spacing has a reserved margin of 0.007 to 0.43 mm
  (Δ' 1.94 to 1.57 mm). That is a spacing change, so it is a ruling.
- **Recommendation.** Do not rerun C8 for the star. Apply the guard fix
  and the miss counts only as an instrument, if the lead wants the
  pocket number. Drop the circular lead-in. The only open option is the
  growing bite with a reserved band margin. It needs an operator ruling
  and a lattice check first (see "Risks").

## Q1. The trace

### The guard chain, in order

| # | Guard (609ce75b) | Rejects when | Counted as |
|---|---|---|---|
| 1 | `lead_band` `:2786` | the arc has no length | fallback |
| 2 | `lead_in_onto` `:2750-2756` | no nearest point, Δ ≤ 0, no pivot | fallback |
| 3 | `lead_in_radius` `:2701` | `m = c·D − w(ρ, Δ) ≤ 0` | fallback |
| 4 | `lead_in_onto` `:2758` | `ρ_L > ρ − Δ/2` | fallback |
| 5 | `lead_in_onto` `:2771-2773` | no traced point within one cell of `a` by θ = π/2 | fallback |
| 6 | `lead_band` `:2815` | `max(total/2, span) ≥ total` | fallback |
| 7 | `lead_band` `:2834` | a piece leaves the region | fallback |

All seven give the same `None`. `lead_legs` (`:2895-2896`) and
`morph_plan` (`:3166`) count `lead_fallback` and `closed_lead_fallback`
only. No reason is kept.

### The sources are correct (no unit, sign or frame bug)

- The ρ source is correct. `ρ = |p − pivot_near(p)|` (`:2755-2756`,
  `pivot_near` `:725`). In the seed-visible zone the pivot is the seed
  centre, so ρ = R + T. This is the same ρ that `curvature_levels` uses.
  Behind a wall, ρ is the distance to the bend node, which is correct
  too.
- The units are correct. `ceiling_width` is `pass.ceiling * 2.0 * r`
  (`:3471`), in mm (2.1756). It is not the fraction 0.3626.
- The sign and frame are correct. `n` points from P to the foot on `a`,
  which is the cleared side. The circle centre is `p + ρ_L n`, toward
  the pivot, as the pivot-frame bound needs. `at(θ)` moves toward `a`
  and back toward E (−t).
- In the model, the C8 radii agree with the C8 table: ρ_L 22.80, 22.95,
  23.13 and 23.88 mm at ρ 25, 30, 35 and 60 mm.

### Where each band stops (model; s 2, R 3)

The Python model rebuilds the two shapes and the field, then ports
`lead_band` and `lead_in_onto` one to one (method below). Star lobe
bands, orientation E = first end. The other orientation gives the same
counts.

| Star level | ρ (mm) | Δ (mm) | m (mm) | ρ_L (mm) | arc `b` (mm) | ideal span (mm) | Stops at |
|---|---|---|---|---|---|---|---|
| 9 | 19.06 | 1.968 | +0.0000 | 19.06 (= ρ) | 14.2–15.5 | — | #4 tight |
| 10 | 21.05 | 1.986 | ±0.0000 | — or = ρ | 12.4–13.5 | — | #3 / #4 |
| 11 | 23.05 | 2.000 | +0.0021 | 22.78 | 10.6–11.7 | — (22.78 > 22.05) | #4 tight |
| 12 | 25.05 | 2.000 | +0.0159 | 22.80 | 8.9–10.2 | 27.5 | #5 walks off `a` |
| 13 | 27.05 | 2.000 | +0.0277 | 22.85 | 7.2–8.7 | 21.2 | #5 |
| 14 | 29.05 | 2.000 | +0.0379 | 22.91 | 5.8–7.2 | 18.0 | #5 |
| 15 | 31.05 | 2.000 | +0.0468 | 22.99 | 4.2–5.9 | 16.2 | #5 |
| 16 | 33.05 | 2.000 | +0.0546 | 23.06 | 2.8–4.6 | 15.2 | #5 |

The "ideal span" is the chord from P to the traced start on exact circles
when the cut arc does not end (`spans.py`). Star totals: 39 bands. 24
stop at #5, 12 at #4 and 3 at #3. There are 0 lead-ins. Pocket: 12 lobe
bands. 10 stop at #3 and 2 at #4. There are 0 lead-ins. The pocket lobes
are at levels 6–10 (ρ 13–21, inside ρ\*) and behind the island (levels
20–23, ρ 12.6–14.5 from the bend pivot, m −0.10 to −0.14).

Closed wraps. The seed clearance is 16.1 mm on the star and 10.85 mm on
the pocket (tool-centre region). So every true closed front after the
core is at ρ ≤ 16.1 mm, which is inside ρ\*, and stops at #3 or #4. The
model finds no other closed piece except slivers of 0.16–0.5 mm. The
harness counts 10 and 9 closed fallbacks. These must be lattice loops
inside ρ\* or slivers. The debug print below settles this.

### Classification

- **#1 (readout): a BUG in the report, not in the plan.** The radius is
  logged only on success.
- **#3 and #4 inside ρ\*: a LIMIT by construction.** C8 predicted it
  ("inside ρ\* the law leaves m = 0"). It is the cause for every closed
  wrap, all pocket lobe bands and 15 of 39 star bands.
- **#5 beyond ρ\*: a BUG that hides a LIMIT.**
  - The bug: the span is read by a trace at mid-arc, and that trace
    needs `span ≤ total/2` (unless `a` reaches past E). So guard #6 and
    the P-shift at `:2814` cannot do their job.
  - The limit: on the star, the arm arcs beyond ρ\* (≤ 11.7 mm) are
    shorter than the ideal span at every level (≥ 13.3 mm), so no P
    exists anywhere on the arc. With the fix, the star still gives 0.
  - The bug alone: on the pocket, the long arcs at levels 13–16 and
    25–26 (17–31 mm at ρ 24–33) have a valid P past mid-arc. In the
    model these arcs are not lobe bands, because they have 2 or 3
    parents. The harness lobe set (5 lobes of 31 arcs) differs from the
    model set (3 of 15), so the verifier must read the real count.

## The proposed patch (against 609ce75b; NOT applied)

- `lead_in_onto` returns `Result<_, LeadMiss>` with the reason, ρ and m.
- `lead_band` scans P from mid-arc to F in steps of one cell. The first
  P with a traced start on `a` wins. It records the mid-arc miss when no
  P exists.
- `lead_closed` records its miss.
- `LobeReport` gains the miss counts, the ρ range and the largest margin.
- The harness prints one line, `lead-in misses piece i: …`.
- No new constant: the scan step is the cell, and the scan ends at F.
- `rustfmt --edition 2024 --check` is clean on the patched copy.
  `git apply --check -p1` passes on 609ce75b, and on the working file as
  of 07:40. The patch is not compiled.

```diff
diff -ru a/crates/rs_cam_core/src/adaptive/research_geodesic.rs b/crates/rs_cam_core/src/adaptive/research_geodesic.rs
--- a/crates/rs_cam_core/src/adaptive/research_geodesic.rs
+++ b/crates/rs_cam_core/src/adaptive/research_geodesic.rs
@@ -2460,6 +2460,72 @@
     /// ... and the smallest and largest lead-in radius used (mm).
     pub lead_radius_min: f64,
     pub lead_radius_max: f64,
+    /// C10: why the lead-ins fell back, one count per band or closed wrap
+    /// (for a band, the reason read at mid-arc): no margin (`m ≤ 0`, all
+    /// of `ρ < ρ*` under the law), `ρ_L > ρ - Δ/2` (the lead circle cannot
+    /// reach the cut band), no traced start within one cell of the cut
+    /// wrap by a right angle (in a band: the lead-in is longer than the
+    /// cut arc behind P), a piece outside the region, and the rest (no
+    /// nearest point, no pivot) ...
+    pub lead_miss_margin: usize,
+    pub lead_miss_tight: usize,
+    pub lead_miss_start: usize,
+    pub lead_miss_outside: usize,
+    pub lead_miss_other: usize,
+    /// ... and over those misses the range of the front radius `ρ` (mm)
+    /// and the largest margin `m = c·D - w(ρ, Δ)` (mm).
+    pub lead_miss_rho_min: f64,
+    pub lead_miss_rho_max: f64,
+    pub lead_miss_margin_max: f64,
+}
+
+/// Why a lead-in has no plan (C10, [`LobeReport`]).
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+enum LeadMissKind {
+    Margin,
+    Tight,
+    Start,
+    Outside,
+    Other,
+}
+
+/// A lead-in miss and where it was read: the front radius `ρ` and the
+/// margin `m` (mm), both 0 when the miss comes before them.
+#[derive(Debug, Clone, Copy)]
+struct LeadMiss {
+    kind: LeadMissKind,
+    rho: f64,
+    margin: f64,
+}
+
+impl LeadMiss {
+    const OTHER: Self = Self {
+        kind: LeadMissKind::Other,
+        rho: 0.0,
+        margin: 0.0,
+    };
+}
+
+impl LobeReport {
+    /// Count one lead-in miss (C10).
+    fn lead_miss(&mut self, miss: LeadMiss) {
+        match miss.kind {
+            LeadMissKind::Margin => self.lead_miss_margin += 1,
+            LeadMissKind::Tight => self.lead_miss_tight += 1,
+            LeadMissKind::Start => self.lead_miss_start += 1,
+            LeadMissKind::Outside => self.lead_miss_outside += 1,
+            LeadMissKind::Other => self.lead_miss_other += 1,
+        }
+        if miss.rho > 0.0 {
+            self.lead_miss_rho_min = if self.lead_miss_rho_min > 0.0 {
+                self.lead_miss_rho_min.min(miss.rho)
+            } else {
+                miss.rho
+            };
+            self.lead_miss_rho_max = self.lead_miss_rho_max.max(miss.rho);
+            self.lead_miss_margin_max = self.lead_miss_margin_max.max(miss.margin);
+        }
+    }
 }
 
 /// `pts` resampled at `m + 1` points of equal arc length, its two ends
@@ -2741,22 +2807,30 @@
 
 /// The lead-in onto `next` at its point `p` heading `t` (unit), from the
 /// cleared wrap `prev` (C8): its radius ([`lead_in_radius_pivot`]) and its
-/// points from its start to `p`, or `None`. The start is traced: walk the
-/// lead circle out from `p` in steps of one cell until a point lies within
-/// one cell of `prev` (the bite is then under one cell); a lead-in that
-/// turns more than a right angle first is not a lead-in (`None`). The
-/// circle reaches `prev` only when `ρ_L ≤ ρ - Δ/2`.
-fn lead_in_onto(ctx: &LeadContext<'_>, prev: &[P2], p: P2, t: P2) -> Option<(f64, Vec<P2>)> {
-    let (foot, delta) = nearest_on(prev, p)?;
+/// points from its start to `p`, or why not ([`LeadMiss`]). The start is
+/// traced: walk the lead circle out from `p` in steps of one cell until a
+/// point lies within one cell of `prev` (the bite is then under one cell);
+/// a lead-in that turns more than a right angle first is not a lead-in.
+/// The circle reaches `prev` only when `ρ_L ≤ ρ - Δ/2`.
+fn lead_in_onto(
+    ctx: &LeadContext<'_>,
+    prev: &[P2],
+    p: P2,
+    t: P2,
+) -> Result<(f64, Vec<P2>), LeadMiss> {
+    let (foot, delta) = nearest_on(prev, p).ok_or(LeadMiss::OTHER)?;
     if delta <= 0.0 {
-        return None;
+        return Err(LeadMiss::OTHER);
     }
     let n = P2::new((foot.x - p.x) / delta, (foot.y - p.y) / delta);
-    let pivot = ctx.field.pivot_near(p)?;
+    let pivot = ctx.field.pivot_near(p).ok_or(LeadMiss::OTHER)?;
     let rho = (p.x - pivot.x).hypot(p.y - pivot.y);
-    let radius = lead_in_radius_pivot(ctx.tool_radius, ctx.ceiling_width, rho, delta)?;
+    let margin = ctx.ceiling_width - circular_front_width(ctx.tool_radius, rho, delta);
+    let miss = |kind: LeadMissKind| LeadMiss { kind, rho, margin };
+    let radius = lead_in_radius_pivot(ctx.tool_radius, ctx.ceiling_width, rho, delta)
+        .ok_or(miss(LeadMissKind::Margin))?;
     if radius > rho - delta / 2.0 {
-        return None;
+        return Err(miss(LeadMissKind::Tight));
     }
     let centre = P2::new(p.x + radius * n.x, p.y + radius * n.y);
     let at = |th: f64| {
@@ -2769,21 +2843,29 @@
     let dtheta = ctx.cell / radius;
     let max_steps = (std::f64::consts::FRAC_PI_2 / dtheta).floor() as usize;
     let reached = (1..=max_steps)
-        .find(|&i| nearest_on(prev, at(i as f64 * dtheta)).is_some_and(|(_, d)| d <= ctx.cell))?;
+        .find(|&i| nearest_on(prev, at(i as f64 * dtheta)).is_some_and(|(_, d)| d <= ctx.cell))
+        .ok_or(miss(LeadMissKind::Start))?;
     let pts: Vec<P2> = (0..=reached).rev().map(|i| at(i as f64 * dtheta)).collect();
-    Some((radius, pts))
+    Ok((radius, pts))
 }
 
 /// One band of a lobe by the lead-in (C8, [`LobeMorph::Lead`]). `a` is the
 /// cut arc `A_k` and `b` the next arc `A_{k+1}`, both with index 0 at the
 /// end E where the cutter stands. Returns the pieces (link, lead-in, cut
 /// P→F, link back, cut P→E: the cutter ends at E of `b`), the lead-in
-/// radius and the link length, or `None` (the seam join then).
-fn lead_band(ctx: &LeadContext<'_>, a: &[P2], b: &[P2]) -> Option<(Vec<PlanPiece>, f64, f64)> {
+/// radius and the link length, or `None` (the seam join then; the miss is
+/// counted in `report`).
+fn lead_band(
+    ctx: &LeadContext<'_>,
+    a: &[P2],
+    b: &[P2],
+    report: &mut LobeReport,
+) -> Option<(Vec<PlanPiece>, f64, f64)> {
     let sub_a = subdivide(a, ctx.cell);
     let sub_b = subdivide(b, ctx.cell);
     let total = polyline_length(&sub_b);
     if total <= 0.0 || sub_b.len() < 2 {
+        report.lead_miss(LeadMiss::OTHER);
         return None;
     }
     // The point at arc length `at` along `sub_b`, its segment index and
@@ -2804,19 +2886,33 @@
         }
         None
     };
-    // P: mid-arc, or further from E when the lead-in's span (traced at
-    // mid-arc) needs it.
-    let (mid, _, t_mid) = point_at(total / 2.0)?;
-    let (_, mid_lead) = lead_in_onto(ctx, a, mid, t_mid)?;
-    let span = mid_lead
-        .first()
-        .map_or(f64::INFINITY, |q| (q.x - mid.x).hypot(q.y - mid.y));
-    let at = (total / 2.0).max(span);
-    if at >= total {
-        return None;
+    // P: the first point from mid-arc toward F, one cell at a time, whose
+    // traced lead-in start lies on `a`. C8 traced at mid-arc only and
+    // returned on a miss there; when the span is over half the arc that
+    // trace walks off the end of `a`, so P never moved (C10).
+    let steps = ((total / 2.0) / ctx.cell).ceil() as usize;
+    let mut first_miss: Option<LeadMiss> = None;
+    let mut found: Option<(P2, usize, f64, Vec<P2>)> = None;
+    for i in 0..steps {
+        let Some((p, seg, t)) = point_at(total / 2.0 + i as f64 * ctx.cell) else {
+            break;
+        };
+        match lead_in_onto(ctx, a, p, t) {
+            Ok((radius, lead)) => {
+                found = Some((p, seg, radius, lead));
+                break;
+            }
+            Err(miss) => {
+                if first_miss.is_none() {
+                    first_miss = Some(miss);
+                }
+            }
+        }
     }
-    let (p, seg, t) = point_at(at)?;
-    let (radius, lead) = lead_in_onto(ctx, a, p, t)?;
+    let Some((p, seg, radius, lead)) = found else {
+        report.lead_miss(first_miss.unwrap_or(LeadMiss::OTHER));
+        return None;
+    };
     let start = *lead.first()?;
     // The link along `a` from E to the lead-in start.
     let iq = super::spiral::nearest_index(&sub_a, start);
@@ -2832,6 +2928,10 @@
         .chain(sub_b.iter().take(seg + 1).rev().copied())
         .collect();
     if !all_inside(ctx.region, &[&link, &lead, &forward, &backward]) {
+        report.lead_miss(LeadMiss {
+            kind: LeadMissKind::Outside,
+            ..LeadMiss::OTHER
+        });
         return None;
     }
     let link_len = polyline_length(&link) + polyline_length(&back);
@@ -2880,7 +2980,7 @@
             a.reverse();
             b.reverse();
         }
-        match lead_band(ctx, &a, &b) {
+        match lead_band(ctx, &a, &b, report) {
             Some((pieces, radius, link_len)) => {
                 report.lead_ins += 1;
                 report.link_length_mm += link_len;
@@ -2916,6 +3016,7 @@
     ctx: &LeadContext<'_>,
     prev: &[P2],
     next: &Chain,
+    report: &mut LobeReport,
 ) -> Option<(Vec<PlanPiece>, Vec<P2>, f64, f64)> {
     let (&seam, &before) = (prev.last()?, prev.get(prev.len().checked_sub(2)?)?);
     let mut lap = next.oriented(seam);
@@ -2931,7 +3032,13 @@
         return None;
     }
     let t = P2::new((second.x - p.x) / len, (second.y - p.y) / len);
-    let (radius, lead) = lead_in_onto(ctx, prev, p, t)?;
+    let (radius, lead) = match lead_in_onto(ctx, prev, p, t) {
+        Ok(found) => found,
+        Err(miss) => {
+            report.lead_miss(miss);
+            return None;
+        }
+    };
     let start = *lead.first()?;
     let mut back: Vec<P2> = prev.to_vec();
     back.reverse();
@@ -2940,6 +3047,10 @@
     let mut link: Vec<P2> = sub.iter().take(iq + 1).copied().collect();
     link.push(start);
     if !all_inside(ctx.region, &[&link, &lead, &lap]) {
+        report.lead_miss(LeadMiss {
+            kind: LeadMissKind::Outside,
+            ..LeadMiss::OTHER
+        });
         return None;
     }
     let link_len = polyline_length(&link);
@@ -3146,7 +3257,7 @@
                 .as_ref()
                 .filter(|(level, _)| level + 1 == first.level);
             if let Some((_, prev)) = after_closed {
-                match lead_closed(&lead, prev, first) {
+                match lead_closed(&lead, prev, first, &mut report) {
                     Some((pieces, lap, radius, link_len)) => {
                         report.closed_lead_ins += 1;
                         report.link_length_mm += link_len;
diff -ru a/crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs b/crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
--- a/crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
+++ b/crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
@@ -578,6 +578,18 @@
                     b.lead_radius_min,
                     b.lead_radius_max
                 );
+                let _ = writeln!(
+                    t,
+                    "lead-in misses piece {i}: no margin {}, rho_L > rho - delta/2 {}, no start on the cut wrap {}, outside the region {}, other {}; rho {:.2} to {:.2} mm, largest margin {:.4} mm",
+                    b.lead_miss_margin,
+                    b.lead_miss_tight,
+                    b.lead_miss_start,
+                    b.lead_miss_outside,
+                    b.lead_miss_other,
+                    b.lead_miss_rho_min,
+                    b.lead_miss_rho_max,
+                    b.lead_miss_margin_max
+                );
             }
         }
     }
```

## Debug prints for the verifier

The patch is the print, so no `eprintln!` is needed (the workspace
denies `print_stderr`). Apply it, then run the harness as in D8:

```text
cargo test --release -p rs_cam_core --features research,test-support \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

Read the two lines per shape for `geodesic_curv_lead`: `lead-ins piece 0`
and `lead-in misses piece 0`.

| Outcome | Meaning |
|---|---|
| Star: no margin + tight ≈ 10–15, no start ≈ 15–25, lead-ins 0, ρ 17–37 mm, largest margin ≈ 0.05–0.07 mm | PREDICTED. The limit: arm arcs are shorter than the span. Drop C8 on the star. |
| Star: lead-ins > 0 | The model under-reads the arm arcs. Read those bands in the SVG (L entries) before any claim. |
| Pocket: no margin dominates, lead-ins 0–2, ρ max ≤ 23 mm | PREDICTED (the model's lobes). The lobe rule keeps the long arcs out of the lobes. |
| Pocket: lead-ins 5 or more | The harness links the long arcs (levels 13–16) into lobes. The fix is real there. Read the `clearing lead` row next. |
| Closed: every closed miss is no margin / tight, ρ ≤ 16.1 mm (star) or ≤ 10.9 mm (pocket) | PREDICTED. The limit inside ρ\*. |
| Closed: a closed miss at ρ > 22.8 mm with "no start" | A small lattice loop, shorter than the span. It is also a limit. |
| Any "other" count > 0 | No pivot or no nearest point: a field defect. Report it. |

## Q2. Where can a growing-bite lead-in fit?

### The form and its span (derived)

- The path leaves the cut arc `a` with bite 0. Its bite `b` grows along
  the arc until it reaches the next arc at `b = Δ`.
- At each bite, the tilt is the largest one that holds the ceiling:
  `b + R(1 − cos β) + h sin β + e ≤ c·D`, with `h = h(Δ)` (the
  `EntryLaw` bound, `research_geodesic.rs:1890-1910`) and
  `e = w(ρ, Δ) − Δ` (the circular excess, the same form as the C8 test).
- So `β(b) = φ₀ + asin((c·D − e − b − R)/√(h² + R²))`. The span is
  `L = ∫₀^Δ db / tan β(b)`.
- Check: on a straight edge (e = 0), L = 7.70 mm. This is the `EntryLaw`
  doc value 7.7 mm.
- Near `b = Δ`, the tilt goes to about `m/h`, so L grows as `h·ln(1/m)`.
  Inside ρ\* (m = 0) there is no finite span.
- The fit condition is `L ≤` the arc length. P may then be at F: the
  ramp runs E → F, and the cut back F → E takes the wedge. Its bite
  grows 0 → Δ under the same pairs (b, β), so the cut back holds the
  same bound. There are no links and no entries at a wall.

### Spans against arcs (model; spans are exact formula values)

| Level | ρ (mm) | m (mm) | circular span (mm) | growing span (mm) | star arm arcs (mm) | pocket seed-visible arcs (mm) |
|---|---|---|---|---|---|---|
| 8 | 17.10 | 0 | — | — (m = 0) | 16.4–17.7 | 10.5–20.8 |
| 9 | 19.06 | 0 | — | — | 14.2–15.5 | 9.8–21.3 |
| 10 | 21.05 | 0 | — | — | 12.4–13.5 | 9.4–22.1 |
| 11 | 23.05 | 0.0021 | — (tight) | 20.4 | 10.6–11.7 | 9.1–23.0 |
| 12 | 25.05 | 0.0159 | 27.5 | 14.4 | 8.9–10.2 | 9.0–23.9 |
| 13 | 27.05 | 0.0277 | 21.2 | 12.8 | 7.2–8.7 | 8.7–24.9 |
| 14 | 29.05 | 0.0379 | 18.0 | 11.9 | 5.8–7.2 | 8.5–25.9 |
| 15 | 31.05 | 0.0468 | 16.2 | 11.3 | 4.2–5.9 | 8.2–26.7 |
| 16 | 33.05 | 0.0546 | 15.2 | 10.9 | 2.8–4.6 | 8.0–16.7 |
| 17 | 35.05 | 0.0615 | 14.3 | 10.6 | 1.2–3.2 | 5.9–7.9 |
| 18 | 37.05 | 0.0676 | 13.8 | 10.3 | 1.1–1.9 | 1.5–7.7 |

On the star the arcs shrink faster than the span: an arm narrows to its
tip. So at every level the arc is shorter than the growing span, by 1.2×
(level 12) to 8× (level 18).

### Fit fractions (model)

| Set | Circular, as coded | Circular, fixed | Growing bite at Δ = s |
|---|---|---|---|
| Star lobe bands | 0 / 39 | 0 / 39 | **0 / 39** |
| Star open arcs with a parent (×2 ends) | 0 / 98 | 0 / 98 | 0 / 98 |
| Pocket lobe bands | 0 / 12 | 0 / 12 | **0 / 12** |
| Pocket open arcs with a parent (×2 ends) | 1 / 158 | 10 / 158 | 18 / 158 |

### What a reserved margin buys (a ruling; not built)

The spacing Δ' at which the growing span equals the shortest star arm arc
of that level (`reserve.py`):

| Level | ρ (mm) | shortest arc (mm) | Δ' needed (mm) | spacing now (mm) |
|---|---|---|---|---|
| 8 | 17.10 | 16.4 | 1.938 | 1.945 (law) |
| 9 | 19.06 | 14.2 | 1.952 | 1.968 (law) |
| 10 | 21.05 | 12.4 | 1.957 | 1.986 (law) |
| 11 | 23.05 | 10.6 | 1.947 | 2.000 |
| 12 | 25.05 | 8.9 | 1.915 | 2.000 |
| 13 | 27.05 | 7.2 | 1.848 | 2.000 |
| 14 | 29.05 | 5.8 | 1.752 | 2.000 |
| 15 | 31.05 | 4.2 | 1.568 | 2.000 |

- Levels 8–15 are 8 bands per arm, so about 40 bands on 5 arms. This is
  the same order as the 58 star wrap-end joins (PREDICTION). Levels
  16–18 (arcs under 4.6 mm) stay seam joins.
- The cost is about one extra level per arm (the mean spacing over
  ρ 17–31 falls by about 0.1 mm).
- **Conclusion for Q2.** At the current spacing the growing bite cannot
  reduce the 58 star joins: the fit is 0. With a reserved margin it could
  reduce them materially. This is a spacing change, so it needs a ruling.

### Risks of the reserved-margin option (read before any ruling)

- The margins are 0.007–0.43 mm, and near full bite the tilt is about
  `m/h` (0.0025 rad at m 0.007). The lattice reads a side cut up to one
  cell (0.5 mm) narrow (`PassLoad::swept_width` doc), so the replay
  cannot resolve such a tilt. The lattice, not this bound, decides the
  inner levels.
- The bound adds the straight-edge tilt term and the circular excess at
  Δ, as the C8 test does. It is not an exact-circle check. Before a
  build, check one band on the exact geometry
  (`print_exact_width_along_move`).
- The fit uses the whole arc (P at F). Meeting-line trims shorten some
  arcs by one stepover (`meeting_trim`), which the model does not apply.

## Method and its limits

Scripts, all under
`phase2/c10/` (copied from the session scratch dir by the lead)
(`python3 -I <script> <that dir>`):

- `model.py` holds the shapes (`fixtures/demo_star.svg`,
  `fixtures/demo_pocket.svg`) and the tool-centre regions (inset R).
  The seed is the EDT maximum of the part, and the model reproduces the
  harness core counts: pocket 4 spiral levels, star 7 (the harness has 6).
  The geodesic field is exact on a 0.1 mm grid: seed visibility, else
  one wrap round a convex disc. The disc is the island grown by R, or a
  reflex star vertex with radius R. The pivot is the tangent point. The
  model also holds the law levels (they match the C5 table), the contour
  arcs split at branch changes, the lobe rules (one child, one parent,
  open, same branch, ratio 1.4, gap ≤ reach 3.207) and a one-to-one
  port of `lead_band` and `lead_in_onto`.
- `fixed.py` holds the fixed scan. `spans.py` and `reserve.py` hold the
  tables.
- `assert_c10.py` asserts each model number this note quotes. It passes.
- `patch/c10.diff` is the patch.

Limits:

- The planner field is first order on a 0.5 mm lattice with lattice
  contours. The model is exact. So the arc sets differ: star 5 lobes of
  44 arcs against 6 of 34, and pocket 3 of 15 against 5 of 31. The
  reason counts are predictions; the harness readout decides.
- The geometry of each arc (ρ, length, margin) does not depend on the
  lattice to first order. The star conclusion has a margin of 1.2× or
  more at every level, so it does not depend on the lobe set.
