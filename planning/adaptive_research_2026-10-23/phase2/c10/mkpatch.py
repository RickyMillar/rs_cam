"""C10: build the proposed patch (not applied) as exact text replacements
on the 609ce75b copies in patch/b."""
import sys
d = sys.argv[1]
src = open(f"{d}/b/research_geodesic.rs").read()
har = open(f"{d}/b/adaptive_geodesic_2d_renders_g1.rs").read()

def rep(text, old, new):
    assert text.count(old) == 1, old[:80]
    return text.replace(old, new)

# 1. report fields
src = rep(src, """    /// ... and the smallest and largest lead-in radius used (mm).
    pub lead_radius_min: f64,
    pub lead_radius_max: f64,
}
""", """    /// ... and the smallest and largest lead-in radius used (mm).
    pub lead_radius_min: f64,
    pub lead_radius_max: f64,
    /// C10: why the lead-ins fell back, one count per band or closed wrap
    /// (for a band, the reason read at mid-arc): no margin (`m ≤ 0`, all
    /// of `ρ < ρ*` under the law), `ρ_L > ρ - Δ/2` (the lead circle cannot
    /// reach the cut band), no traced start within one cell of the cut
    /// wrap by a right angle (in a band: the lead-in is longer than the
    /// cut arc behind P), a piece outside the region, and the rest (no
    /// nearest point, no pivot) ...
    pub lead_miss_margin: usize,
    pub lead_miss_tight: usize,
    pub lead_miss_start: usize,
    pub lead_miss_outside: usize,
    pub lead_miss_other: usize,
    /// ... and over those misses the range of the front radius `ρ` (mm)
    /// and the largest margin `m = c·D - w(ρ, Δ)` (mm).
    pub lead_miss_rho_min: f64,
    pub lead_miss_rho_max: f64,
    pub lead_miss_margin_max: f64,
}

/// Why a lead-in has no plan (C10, [`LobeReport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeadMissKind {
    Margin,
    Tight,
    Start,
    Outside,
    Other,
}

/// A lead-in miss and where it was read: the front radius `ρ` and the
/// margin `m` (mm), both 0 when the miss comes before them.
#[derive(Debug, Clone, Copy)]
struct LeadMiss {
    kind: LeadMissKind,
    rho: f64,
    margin: f64,
}

impl LeadMiss {
    const OTHER: Self = Self {
        kind: LeadMissKind::Other,
        rho: 0.0,
        margin: 0.0,
    };
}

impl LobeReport {
    /// Count one lead-in miss (C10).
    fn lead_miss(&mut self, miss: LeadMiss) {
        match miss.kind {
            LeadMissKind::Margin => self.lead_miss_margin += 1,
            LeadMissKind::Tight => self.lead_miss_tight += 1,
            LeadMissKind::Start => self.lead_miss_start += 1,
            LeadMissKind::Outside => self.lead_miss_outside += 1,
            LeadMissKind::Other => self.lead_miss_other += 1,
        }
        if miss.rho > 0.0 {
            self.lead_miss_rho_min = if self.lead_miss_rho_min > 0.0 {
                self.lead_miss_rho_min.min(miss.rho)
            } else {
                miss.rho
            };
            self.lead_miss_rho_max = self.lead_miss_rho_max.max(miss.rho);
            self.lead_miss_margin_max = self.lead_miss_margin_max.max(miss.margin);
        }
    }
}
""")

# 2. lead_in_onto returns why it misses
src = rep(src, """/// points from its start to `p`, or `None`. The start is traced: walk the
/// lead circle out from `p` in steps of one cell until a point lies within
/// one cell of `prev` (the bite is then under one cell); a lead-in that
/// turns more than a right angle first is not a lead-in (`None`). The
/// circle reaches `prev` only when `ρ_L ≤ ρ - Δ/2`.
fn lead_in_onto(ctx: &LeadContext<'_>, prev: &[P2], p: P2, t: P2) -> Option<(f64, Vec<P2>)> {
    let (foot, delta) = nearest_on(prev, p)?;
    if delta <= 0.0 {
        return None;
    }
    let n = P2::new((foot.x - p.x) / delta, (foot.y - p.y) / delta);
    let pivot = ctx.field.pivot_near(p)?;
    let rho = (p.x - pivot.x).hypot(p.y - pivot.y);
    let radius = lead_in_radius_pivot(ctx.tool_radius, ctx.ceiling_width, rho, delta)?;
    if radius > rho - delta / 2.0 {
        return None;
    }
""", """/// points from its start to `p`, or why not ([`LeadMiss`]). The start is
/// traced: walk the lead circle out from `p` in steps of one cell until a
/// point lies within one cell of `prev` (the bite is then under one cell);
/// a lead-in that turns more than a right angle first is not a lead-in.
/// The circle reaches `prev` only when `ρ_L ≤ ρ - Δ/2`.
fn lead_in_onto(
    ctx: &LeadContext<'_>,
    prev: &[P2],
    p: P2,
    t: P2,
) -> Result<(f64, Vec<P2>), LeadMiss> {
    let (foot, delta) = nearest_on(prev, p).ok_or(LeadMiss::OTHER)?;
    if delta <= 0.0 {
        return Err(LeadMiss::OTHER);
    }
    let n = P2::new((foot.x - p.x) / delta, (foot.y - p.y) / delta);
    let pivot = ctx.field.pivot_near(p).ok_or(LeadMiss::OTHER)?;
    let rho = (p.x - pivot.x).hypot(p.y - pivot.y);
    let margin = ctx.ceiling_width - circular_front_width(ctx.tool_radius, rho, delta);
    let miss = |kind: LeadMissKind| LeadMiss { kind, rho, margin };
    let radius = lead_in_radius_pivot(ctx.tool_radius, ctx.ceiling_width, rho, delta)
        .ok_or(miss(LeadMissKind::Margin))?;
    if radius > rho - delta / 2.0 {
        return Err(miss(LeadMissKind::Tight));
    }
""")
src = rep(src, """    let reached = (1..=max_steps)
        .find(|&i| nearest_on(prev, at(i as f64 * dtheta)).is_some_and(|(_, d)| d <= ctx.cell))?;
    let pts: Vec<P2> = (0..=reached).rev().map(|i| at(i as f64 * dtheta)).collect();
    Some((radius, pts))
}
""", """    let reached = (1..=max_steps)
        .find(|&i| nearest_on(prev, at(i as f64 * dtheta)).is_some_and(|(_, d)| d <= ctx.cell))
        .ok_or(miss(LeadMissKind::Start))?;
    let pts: Vec<P2> = (0..=reached).rev().map(|i| at(i as f64 * dtheta)).collect();
    Ok((radius, pts))
}
""")

# 3. lead_band: scan P from mid-arc toward F (the fix), record the miss
src = rep(src, """/// radius and the link length, or `None` (the seam join then).
fn lead_band(ctx: &LeadContext<'_>, a: &[P2], b: &[P2]) -> Option<(Vec<PlanPiece>, f64, f64)> {
    let sub_a = subdivide(a, ctx.cell);
    let sub_b = subdivide(b, ctx.cell);
    let total = polyline_length(&sub_b);
    if total <= 0.0 || sub_b.len() < 2 {
        return None;
    }
""", """/// radius and the link length, or `None` (the seam join then; the miss is
/// counted in `report`).
fn lead_band(
    ctx: &LeadContext<'_>,
    a: &[P2],
    b: &[P2],
    report: &mut LobeReport,
) -> Option<(Vec<PlanPiece>, f64, f64)> {
    let sub_a = subdivide(a, ctx.cell);
    let sub_b = subdivide(b, ctx.cell);
    let total = polyline_length(&sub_b);
    if total <= 0.0 || sub_b.len() < 2 {
        report.lead_miss(LeadMiss::OTHER);
        return None;
    }
""")
src = rep(src, """    // P: mid-arc, or further from E when the lead-in's span (traced at
    // mid-arc) needs it.
    let (mid, _, t_mid) = point_at(total / 2.0)?;
    let (_, mid_lead) = lead_in_onto(ctx, a, mid, t_mid)?;
    let span = mid_lead
        .first()
        .map_or(f64::INFINITY, |q| (q.x - mid.x).hypot(q.y - mid.y));
    let at = (total / 2.0).max(span);
    if at >= total {
        return None;
    }
    let (p, seg, t) = point_at(at)?;
    let (radius, lead) = lead_in_onto(ctx, a, p, t)?;
    let start = *lead.first()?;
""", """    // P: the first point from mid-arc toward F, one cell at a time, whose
    // traced lead-in start lies on `a`. C8 traced at mid-arc only and
    // returned on a miss there; when the span is over half the arc that
    // trace walks off the end of `a`, so P never moved (C10).
    let steps = ((total / 2.0) / ctx.cell).ceil() as usize;
    let mut first_miss: Option<LeadMiss> = None;
    let mut found: Option<(P2, usize, f64, Vec<P2>)> = None;
    for i in 0..steps {
        let Some((p, seg, t)) = point_at(total / 2.0 + i as f64 * ctx.cell) else {
            break;
        };
        match lead_in_onto(ctx, a, p, t) {
            Ok((radius, lead)) => {
                found = Some((p, seg, radius, lead));
                break;
            }
            Err(miss) => {
                if first_miss.is_none() {
                    first_miss = Some(miss);
                }
            }
        }
    }
    let Some((p, seg, radius, lead)) = found else {
        report.lead_miss(first_miss.unwrap_or(LeadMiss::OTHER));
        return None;
    };
    let start = *lead.first()?;
""")
src = rep(src, """    if !all_inside(ctx.region, &[&link, &lead, &forward, &backward]) {
        return None;
    }
""", """    if !all_inside(ctx.region, &[&link, &lead, &forward, &backward]) {
        report.lead_miss(LeadMiss {
            kind: LeadMissKind::Outside,
            ..LeadMiss::OTHER
        });
        return None;
    }
""")
src = rep(src, """        match lead_band(ctx, &a, &b) {""", """        match lead_band(ctx, &a, &b, report) {""")

# 4. lead_closed records its miss
src = rep(src, """fn lead_closed(
    ctx: &LeadContext<'_>,
    prev: &[P2],
    next: &Chain,
) -> Option<(Vec<PlanPiece>, Vec<P2>, f64, f64)> {""", """fn lead_closed(
    ctx: &LeadContext<'_>,
    prev: &[P2],
    next: &Chain,
    report: &mut LobeReport,
) -> Option<(Vec<PlanPiece>, Vec<P2>, f64, f64)> {""")
src = rep(src, """    let t = P2::new((second.x - p.x) / len, (second.y - p.y) / len);
    let (radius, lead) = lead_in_onto(ctx, prev, p, t)?;
    let start = *lead.first()?;
    let mut back: Vec<P2> = prev.to_vec();""", """    let t = P2::new((second.x - p.x) / len, (second.y - p.y) / len);
    let (radius, lead) = match lead_in_onto(ctx, prev, p, t) {
        Ok(found) => found,
        Err(miss) => {
            report.lead_miss(miss);
            return None;
        }
    };
    let start = *lead.first()?;
    let mut back: Vec<P2> = prev.to_vec();""")
src = rep(src, """    if !all_inside(ctx.region, &[&link, &lead, &lap]) {
        return None;
    }""", """    if !all_inside(ctx.region, &[&link, &lead, &lap]) {
        report.lead_miss(LeadMiss {
            kind: LeadMissKind::Outside,
            ..LeadMiss::OTHER
        });
        return None;
    }""")
src = rep(src, """                match lead_closed(&lead, prev, first) {""", """                match lead_closed(&lead, prev, first, &mut report) {""")

# 5. harness: print the misses
har = rep(har, """                    b.lead_radius_min,
                    b.lead_radius_max
                );
            }""", """                    b.lead_radius_min,
                    b.lead_radius_max
                );
                let _ = writeln!(
                    t,
                    "lead-in misses piece {i}: no margin {}, rho_L > rho - delta/2 {}, no start on the cut wrap {}, outside the region {}, other {}; rho {:.2} to {:.2} mm, largest margin {:.4} mm",
                    b.lead_miss_margin,
                    b.lead_miss_tight,
                    b.lead_miss_start,
                    b.lead_miss_outside,
                    b.lead_miss_other,
                    b.lead_miss_rho_min,
                    b.lead_miss_rho_max,
                    b.lead_miss_margin_max
                );
            }""")
open(f"{d}/b/research_geodesic.rs", "w").write(src)
open(f"{d}/b/adaptive_geodesic_2d_renders_g1.rs", "w").write(har)
print("ok")
