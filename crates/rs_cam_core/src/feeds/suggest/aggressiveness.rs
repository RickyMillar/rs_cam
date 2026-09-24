//! Suggest pass 6b: the machine aggressiveness dial (ruling R4, 2026-09-24).
//!
//! The dial sets the LOAD of the cut as a fraction of the load at the base
//! engagement. The chipload does not move. The pass reads the engagement that
//! passes 0 to 6 left (the base), and changes the depth per pass and the
//! stepover by ONE common scale (ruling Q11) until the predicted lateral force
//! and spindle power are at the target. The feed, the plunge and the RPM stay.
//! Pass 9 then re-derives the feed at the final depth through the published
//! depth ladder only, and pass 10 re-checks power.
//!
//! On a 3D Rough with a step ladder (`coarse_steps`), the depth lever is the
//! deepest step, because the load is largest there. The common scale moves
//! every step of the ladder, and the ladder stays valid (D7 of
//! `planning/adaptive3d_step_ladder_roughing_2026-09-24/PLAN.md`). The record
//! gives the deepest step in `dpp_from` / `dpp_to`.
//!
//! The ladder holds the operator's steps (ruling 1 of 2026-09-24, "keep my
//! steps, only cap"). So on a ladder the dial only LOWERS the steps: below
//! 1.0 it scales the whole ladder down to hold the load, and the record
//! states the lowering. Above 1.0 the depth lever is capped at the deepest
//! step as it stands, so no step rises; the stepover alone takes the
//! raise. A step that a scale removes gets a `CoarseStepRemoved` note.
//!
//! The target is `k_eff = k × ld`: `k` is `MachineProfile::aggressiveness`
//! and `ld` is the long-tool share `feeds::long_tool_load_share` (ruling Q7).
//!
//! - `k_eff == 1.0`: the pass does nothing and records nothing.
//! - `k_eff < 1.0`: the largest `s` in `(0, 1]` that fits. A lever that
//!   reaches its floor (`MIN_AP_MM`, `MIN_AE_MM`) stops there and the solve
//!   continues on the other lever. If every lever is at its floor and the
//!   load is still over the target, the warning says `target_met: false`.
//!   The feed is never cut to close the gap.
//! - `k_eff > 1.0` (ruling Q3: warn, do not refuse): the largest `s >= 1`
//!   that fits `k_eff × base`, inside the rigidity depth cap, the flute
//!   length, a stepover of one diameter and the rated spindle power. A depth
//!   that the deflection back-off lowered is not raised again.
//! - A Finish pass (Q4) and a drill (Q5) get a record that says why the dial
//!   did not act.
//!
//! Both loads rise monotonically with the depth and the width, so a
//! bisection on `s` is valid. The force model has an edge intercept
//! (`F = ap · (Ks · h + F_edge)`), so the width is a weak lever on force; the
//! common scale gives the depth most of the force cut (spec §2.3).

use crate::compute::catalog::OperationConfig;
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{MIN_AE_MM, MIN_AP_MM, OperationFamily, PassRole, ToolGeometryHint};
use crate::machine::MachineProfile;
use crate::material::Material;

use super::{AggressivenessShortfall, AggressivenessSkip, SuggestContext, SuggestWarning};

/// Bisection steps for the common scale. 50 halvings of a unit bracket are
/// far below the 0.001 mm write resolution.
const SCALE_BISECTION_STEPS: usize = 50;

/// The dial is "at 1.0" inside this band, so a round trip through a file
/// does not wake the pass.
const UNIT_SHARE_EPS: f64 = 1e-9;

/// Relative slack on the "at or below the target" tests. The loads are
/// re-evaluated at the rounded values that ship, so the comparison needs a
/// float allowance and nothing more.
const TARGET_REL_EPS: f64 = 1e-9;

/// The three load figures at one engagement. `None` means the model refused.
#[derive(Debug, Clone, Copy)]
struct Loads {
    force_n: Option<f64>,
    power_kw: Option<f64>,
    section_mm2: f64,
}

/// Everything the load evaluation reads, fixed for the whole pass.
struct LoadModel<'a> {
    operation: &'a OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
    context: SuggestContext<'a>,
    geom: ToolGeometryHint,
    shank_mm: f64,
    /// The held chipload (mm/tooth): feed / (rpm × flutes).
    fz_mm: f64,
}

impl LoadModel<'_> {
    fn loads(&self, ap: f64, ae: f64) -> Loads {
        let effective_d =
            crate::feeds::effective_diameter(self.geom, self.tool.diameter, self.shank_mm, ap);
        let psi = crate::feeds::force::immersion_angle(ae, effective_d / 2.0);
        let force_n =
            crate::feeds::force::lateral_cutting_force(self.material, ap, psi, self.fz_mm)
                .filter(|f| f.is_finite() && *f > 0.0);
        // The probe cuts at the one step `ap`. On a step ladder a coarse
        // step of the operation would otherwise be the deepest step, and
        // the power model reads the deepest step (D7).
        let mut probe = super::ladder::at_single_step(self.operation, ap);
        probe.set_stepover(ae);
        let power_kw = crate::feeds::power_at_operating_point(
            &probe,
            self.tool,
            self.material,
            self.machine,
            self.context.calculator_operating_point,
        )
        .ok()
        .map(|p| p.required_kw)
        .filter(|p| p.is_finite() && *p > 0.0);
        Loads {
            force_n,
            power_kw,
            section_mm2: self.geom.mrr_cross_section_mm2(ap, ae),
        }
    }

    /// The rated spindle power at the operation's RPM, the machine limit the
    /// raise above 1.0 must also respect.
    fn rated_power_kw(&self) -> Option<f64> {
        let rpm = self
            .operation
            .spindle_rpm()
            .map(f64::from)
            .or_else(|| self.context.calculator_operating_point.map(|c| c.rpm))?;
        let p = self.machine.power_at_rpm(rpm);
        (p.is_finite() && p > 0.0).then_some(p)
    }
}

/// `true` when every modelled load at `now` is at or below `share` x `base`.
/// The proxy counts only when both models refuse.
fn within_share(now: Loads, base: Loads, share: f64) -> bool {
    let at_or_below = |n: Option<f64>, b: Option<f64>| match (n, b) {
        (Some(n), Some(b)) => n <= share * b * (1.0 + TARGET_REL_EPS),
        _ => true,
    };
    let modelled = base.force_n.is_some() || base.power_kw.is_some();
    if modelled {
        at_or_below(now.force_n, base.force_n) && at_or_below(now.power_kw, base.power_kw)
    } else {
        now.section_mm2 <= share * base.section_mm2 * (1.0 + TARGET_REL_EPS)
    }
}

/// One lever: its base value, its floor and its cap.
#[derive(Debug, Clone, Copy)]
struct Lever {
    base: f64,
    floor: f64,
    cap: f64,
}

impl Lever {
    fn at(self, s: f64) -> f64 {
        (s * self.base).clamp(self.floor.min(self.base), self.cap.max(self.base))
    }
}

/// Pass 6b. See the module doc.
pub(super) fn apply_aggressiveness(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    earlier: &[SuggestWarning],
) -> Vec<SuggestWarning> {
    let usable = |v: f64| v.is_finite() && v > 0.0;
    let k = machine.aggressiveness;
    if !usable(k) {
        return Vec::new();
    }
    let ld_factor = crate::feeds::long_tool_load_share(tool.stickout, tool.diameter);
    let target_share = k * ld_factor;
    if (target_share - 1.0).abs() < UNIT_SHARE_EPS {
        return Vec::new();
    }

    let family = operation.op_type().spec().feeds_family;
    if family == OperationFamily::Drill {
        return vec![SuggestWarning::AggressivenessNotApplied {
            aggressiveness: k,
            reason: AggressivenessSkip::Drill,
        }];
    }
    if pass_role == PassRole::Finish {
        return vec![SuggestWarning::AggressivenessNotApplied {
            aggressiveness: k,
            reason: AggressivenessSkip::FinishRole,
        }];
    }

    // Step 2: no calculator operating point (a hand-typed feed through
    // `resolve_operation_invariants`) means no derived chip to hold. Same
    // rule as pass 9.
    let Some(calc) = context.calculator_operating_point else {
        return Vec::new();
    };
    let rpm = operation
        .spindle_rpm()
        .map(f64::from)
        .filter(|v| usable(*v))
        .unwrap_or(calc.rpm);
    let flutes = f64::from(tool.flute_count.max(1));
    let feed = operation.feed_rate();
    if !(usable(rpm) && usable(feed)) {
        return Vec::new();
    }
    let fz_mm = feed / (rpm * flutes);

    // The step ladder (D7): the load that the dial holds is the load at the
    // deepest step, so the depth lever is the deepest step. The dial moves
    // the whole ladder by the one common scale (see the write below). With
    // no ladder the deepest step is `depth_per_pass`.
    let dpp_now = operation.deepest_axial_step().filter(|v| usable(*v));
    let scallop_set_stepover = super::operation_feeds_hints(operation).2.is_some();
    let stepover_now = operation
        .stepover()
        .filter(|v| usable(*v))
        .filter(|_| !scallop_set_stepover);
    let ap0 = dpp_now.unwrap_or(calc.axial_depth_mm);
    let ae0 = operation
        .stepover()
        .filter(|v| usable(*v))
        .unwrap_or(calc.radial_width_mm);
    if !(usable(ap0) && usable(ae0)) {
        return Vec::new();
    }

    let geom = build_cutter(tool).to_geometry_hint();
    let shank_mm = if usable(tool.shank_diameter) {
        tool.shank_diameter
    } else {
        tool.diameter
    };
    let model = LoadModel {
        operation,
        tool,
        material,
        machine,
        context,
        geom,
        shank_mm,
        fz_mm,
    };
    let base = model.loads(ap0, ae0);

    // The levers and their bounds. A raise above 1.0 stays inside the caps
    // that already bound the base engagement.
    // Ruling 1 of 2026-09-24: a ladder holds the operator's steps, and the
    // dial may only lower them. Its depth cap is the deepest step as it
    // stands, so a raise above 1.0 moves the stepover alone.
    let depth_cap = if target_share > 1.0 && !super::ladder::has_ladder(operation) {
        depth_cap_for_raise(operation, tool, machine, pass_role, ap0, earlier)
    } else {
        ap0
    };
    let depth = dpp_now.map(|b| Lever {
        base: b,
        floor: MIN_AP_MM,
        cap: depth_cap,
    });
    let width = stepover_now.map(|b| Lever {
        base: b,
        floor: MIN_AE_MM,
        cap: if target_share > 1.0 { tool.diameter } else { b },
    });

    if depth.is_none() && width.is_none() {
        return vec![engagement_record(
            k,
            ld_factor,
            target_share,
            1.0,
            (None, None),
            (None, None),
            base,
            base,
            false,
            Some(AggressivenessShortfall::NoLever),
        )];
    }

    let engagement_at = |s: f64| {
        (
            depth.map_or(ap0, |l| l.at(s)),
            width.map_or(ae0, |l| l.at(s)),
        )
    };
    // A raise above 1.0 must also stay inside the rated spindle power, a
    // machine limit. Below 1.0 the load only falls, so the test is moot.
    let rated_kw = (target_share > 1.0)
        .then(|| model.rated_power_kw())
        .flatten();
    let fits = |s: f64| {
        let (ap, ae) = engagement_at(s);
        let now = model.loads(ap, ae);
        let under_rated = match (now.power_kw, rated_kw) {
            (Some(p), Some(r)) => p <= r,
            _ => true,
        };
        within_share(now, base, target_share) && under_rated
    };

    let (scale, shortfall) = if target_share < 1.0 {
        if fits(0.0) {
            let mut lo = 0.0_f64;
            let mut hi = 1.0_f64;
            for _ in 0..SCALE_BISECTION_STEPS {
                let mid = 0.5 * (lo + hi);
                if fits(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo, None)
        } else {
            (0.0, Some(AggressivenessShortfall::LeverFloor))
        }
    } else {
        // Above 1.0: beyond `s_hi` every lever sits on its cap.
        let s_hi = [depth, width]
            .into_iter()
            .flatten()
            .map(|l| l.cap.max(l.base) / l.base)
            .fold(1.0_f64, f64::max);
        if s_hi <= 1.0 {
            (1.0, Some(AggressivenessShortfall::EngagementCap))
        } else if fits(s_hi) {
            (s_hi, Some(AggressivenessShortfall::EngagementCap))
        } else {
            let mut lo = 1.0_f64;
            let mut hi = s_hi;
            for _ in 0..SCALE_BISECTION_STEPS {
                let mid = 0.5 * (lo + hi);
                if fits(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo, None)
        }
    };

    // Write the values that ship. The common scale holds on the raw values;
    // the shipped values then round DOWN to 0.001 mm, so the rounding cannot
    // lift the load over the target, and the depth snaps to a step the
    // generator will cut (`realised_step_down`, as the apply funnel does),
    // which is at or below the value asked for. A lever the solve did not
    // move (it sits on its floor or its cap) ships its base unchanged: a
    // snap must not move a value the dial left alone. Above 1.0 a snap that
    // falls under the base keeps the base.
    let (ap_raw, ae_raw) = engagement_at(scale);
    let round = |v: f64| {
        let r = (v / 0.001).floor() * 0.001;
        if r > 0.0 { r } else { v }
    };
    let unmoved = |raw: f64, base: f64| (raw - base).abs() <= base.abs() * 1e-9;
    let raising = target_share > 1.0;
    let total = operation.total_depth();
    let ap_ship = depth.map(|l| {
        if unmoved(ap_raw, l.base) {
            return l.base;
        }
        let r = round(ap_raw);
        let snapped = total
            .and_then(|t| crate::ops::depth::realised_step_down(t, r))
            .filter(|v| usable(*v))
            .unwrap_or(r);
        if raising {
            snapped.max(l.base)
        } else {
            snapped
        }
    });
    let ae_ship = width.map(|l| {
        if unmoved(ae_raw, l.base) {
            return l.base;
        }
        let r = round(ae_raw);
        if raising { r.max(l.base) } else { r }
    });
    let after = model.loads(ap_ship.unwrap_or(ap0), ae_ship.unwrap_or(ae0));
    let mut removal_notes = Vec::new();
    if let Some(ap) = ap_ship {
        if super::ladder::has_ladder(operation) {
            // The step ladder: `ap` is the new deepest step. Every step moves
            // by the factor `ap / ap0`, and each rounds DOWN to 0.001 mm, so
            // no step rises above its scaled value. Above 1.0 the base step
            // does not fall under its old value. A lever the solve did not
            // move leaves the whole ladder as it was.
            if !unmoved(ap, ap0)
                && let Some(base) = operation.depth_per_pass()
            {
                let scaled = round(base * ap / ap0);
                let base_ship = if raising { scaled.max(base) } else { scaled };
                let removed =
                    super::ladder::set_deepest_axial_step(operation, ap, base_ship, &round);
                removal_notes = super::ladder::removal_notes(&removed, "aggressiveness dial");
            }
        } else {
            operation.set_depth_per_pass(ap);
        }
    }
    if let Some(ae) = ae_ship {
        operation.set_stepover(ae);
    }

    let target_met = shortfall.is_none()
        && if target_share < 1.0 {
            within_share(after, base, target_share)
        } else {
            true
        };
    let shortfall = if target_met {
        None
    } else {
        Some(shortfall.unwrap_or(AggressivenessShortfall::LeverFloor))
    };

    let mut records = vec![engagement_record(
        k,
        ld_factor,
        target_share,
        scale,
        (depth.map(|l| l.base), ap_ship),
        (width.map(|l| l.base), ae_ship),
        base,
        after,
        target_met,
        shortfall,
    )];
    records.extend(removal_notes);
    records
}

/// The deepest depth per pass a raise above 1.0 may write: the rigidity
/// depth cap (roughing), the flute length, and, when the deflection back-off
/// lowered the depth, the depth it left.
fn depth_cap_for_raise(
    operation: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    pass_role: PassRole,
    ap0: f64,
    earlier: &[SuggestWarning],
) -> f64 {
    let backed_off = earlier
        .iter()
        .any(|w| matches!(w, SuggestWarning::DppCappedByDeflection { .. }));
    if backed_off {
        return ap0;
    }
    let mut cap = if tool.cutting_length.is_finite() && tool.cutting_length > 0.0 {
        tool.cutting_length
    } else {
        ap0
    };
    let family = operation.op_type().spec().feeds_family;
    let cutter = build_cutter(tool);
    let cap_at = |depth: f64| -> Option<f64> {
        let diameter = crate::feeds::geometry::depth_cap_diameter_mm(&cutter, depth);
        machine
            .rigidity
            .depth_cap_mm(family, pass_role, diameter)
            .map(|c| c.cap_mm())
            .filter(|c| c.is_finite() && *c > 0.0)
    };
    if cap_at(ap0).is_some() {
        // The cap can grow with depth on a tapered ball. Take the deepest
        // depth inside its own cap, below the flute length.
        let inside = |d: f64| cap_at(d).is_some_and(|c| d <= c);
        let mut lo = ap0.min(cap);
        let mut hi = cap;
        if inside(hi) {
            lo = hi;
        } else {
            for _ in 0..SCALE_BISECTION_STEPS {
                let mid = 0.5 * (lo + hi);
                if inside(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
        }
        cap = lo;
    }
    cap.max(ap0)
}

// SAFETY: the record carries every field of the warning; a struct literal at
// each call site would repeat all fourteen fields twice.
#[allow(clippy::too_many_arguments)]
fn engagement_record(
    aggressiveness: f64,
    ld_factor: f64,
    target_share: f64,
    scale: f64,
    dpp: (Option<f64>, Option<f64>),
    stepover: (Option<f64>, Option<f64>),
    before: Loads,
    after: Loads,
    target_met: bool,
    shortfall: Option<AggressivenessShortfall>,
) -> SuggestWarning {
    let proxy = before.force_n.is_none() && before.power_kw.is_none();
    SuggestWarning::EngagementReducedForAggressiveness {
        aggressiveness,
        ld_factor,
        target_share,
        scale,
        dpp_from: dpp.0,
        dpp_to: dpp.1,
        stepover_from: stepover.0,
        stepover_to: stepover.1,
        force_n_before: before.force_n,
        force_n_after: after.force_n.filter(|_| before.force_n.is_some()),
        power_kw_before: before.power_kw,
        power_kw_after: after.power_kw.filter(|_| before.power_kw.is_some()),
        section_mm2_before: proxy.then_some(before.section_mm2),
        section_mm2_after: proxy.then_some(after.section_mm2),
        target_met,
        shortfall,
        applied: true,
    }
}
