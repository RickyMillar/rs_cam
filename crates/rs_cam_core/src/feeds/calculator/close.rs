//! Steps 8 to 9c of the calculator (the plunge, the ramp, the rubbing
//! floor, the drill cycle) and the result.

use super::CalcState;
use crate::feeds::geometry::UnliftedFeed;
use crate::feeds::sourced::{self, Basis, PointBases};
use crate::feeds::vertical::VerticalBinding;
use crate::feeds::{
    ChiploadSource, FeedsDerates, FeedsResult, FeedsWarning, FormulaBreakdown, OperationFamily,
    RampBasis, engagement, plunge, ramp, worst_case_power,
};
use crate::material::FormulaConstant;

/// The plunge and the ramp of Steps 8 and 9.
pub(super) struct Entry {
    ramp: RampBasis,
    plunge_basis: plunge::PlungeBasis,
    /// The plunge (Step 8; Step 9c sets a drill plunge to its feed).
    pub(super) plunge_rate: f64,
    material_plunge_row: FormulaConstant,
}

impl Entry {
    /// P13b: the basis of the Step 8 plunge at the feed of `s`: the basis
    /// that [`result`] gives the plunge when Step 9c does not move it.
    pub(super) fn step8_basis(&self, s: &CalcState) -> Basis {
        plunge_value_basis(
            &self.plunge_basis,
            unlifted_feed(s),
            s.speed.rpm,
            &s.feed.basis,
            &self.material_plunge_row,
        )
    }
}

/// P18 (review D14): the feed of `s` before the radial chip-thinning lift,
/// at the shipped width and engaged diameter. The plunge reads it.
fn unlifted_feed(s: &CalcState) -> UnliftedFeed {
    UnliftedFeed::at_cut(s.feed.feed, s.feed.thinning, s.cut.ae, s.feed.effective_d)
}

/// The basis of the plunge at `feed` and `rpm`: the binding term of the
/// plunge rule, `plunge = feed` for a drill cycle, or the material base.
fn plunge_value_basis(
    plunge_basis: &plunge::PlungeBasis,
    feed: UnliftedFeed,
    rpm: f64,
    feed_basis: &Basis,
    material_plunge_row: &FormulaConstant,
) -> Basis {
    match plunge::resolve_plunge_at(plunge_basis, feed, rpm) {
        Some((_, binding)) => plunge_basis.basis(binding, feed_basis, material_plunge_row.clone()),
        None => match plunge_basis {
            plunge::PlungeBasis::DrillCycle { .. } => {
                Basis::derived("plunge = feed", vec![feed_basis.clone()])
            }
            plunge::PlungeBasis::Claimed { .. } | plunge::PlungeBasis::MaterialBase { .. } => {
                Basis::derived(
                    "material plunge base",
                    vec![Basis::Rule(material_plunge_row.clone())],
                )
            }
        },
    }
}

/// Step 8 (the plunge rule and the ramp basis) and Step 9 (the plunge at
/// the final feed).
pub(super) fn step8_plunge(s: &CalcState) -> Entry {
    let input = s.input;
    let material = input.material;
    // --- Step 8: Plunge rate (G10) ---
    // The plunge is `min(rule, tip cap, axial chip, F)` at the final feed of Step 7
    // (`feeds::plunge`). The rule is the G10 claim, a printed fraction of
    // the side feed (flat end mill F / Z, ball and tapered ball 0.50 F, 60°
    // V-bit F / 3; rulings Q4 and Q5). When no claim covers the tool, the
    // rule is the named repo rule: the material base below, scaled by the
    // diameter. The tip cap of a ball or a tapered ball is a named repo rule
    // too (`tool_load::plunge_stress`, the one producer). P20b: the one
    // vertical-rate limit (`feeds::vertical`) bounds every plunge by the
    // G6 axial chip x rpm x Z where a G6 claim covers the tool, and by the
    // machine Z axis rate where the machine states it, so the ramp basis
    // (Step 8b) comes first. The apply funnel runs the rule again at the
    // feed and the RPM that ship.
    //
    // --- Step 8b: the sourced half of the ramp feed (G6 ramp) ---
    // The G6 drill claim gives the axial chip of a helix or ramp entry, or
    // the record says why there is none. The number needs θ and the shipped
    // feed and RPM, so `suggest::apply` resolves it. No clamp rule remains.
    let ramp = ramp::ramp_basis(input, s.chip.matched_lut_row.as_ref());
    let plunge = material.plunge_rate_base(input.tool_diameter);
    let plunge_basis = plunge::plunge_basis(input, plunge, &ramp);
    let material_plunge_row = material.plunge_rate_base_constant();

    // --- Step 9: no factor (ruling R4, 2026-09-24) ---
    //
    // Until R4 this step multiplied the feed, the plunge and the ramp by
    // `machine.safety_factor` (0.75), the "hidden 25 %". The factor is gone.
    // The feed ships the band chipload times the published depth ladder. The
    // plunge takes no factor either (ruling Q5): a plunge is full-width
    // axial, so no engagement lever acts on it. The plunge
    // is the G10 rule of Step 8 at this feed. A drill cycle gives no value
    // here; Step 9c sets its plunge to the drill feed.
    //
    // P18 (review D14): the rule reads the feed BEFORE the radial
    // chip-thinning lift. The lift corrects a narrow radial width; a plunge
    // is axial and has no radial thinning.
    let plunge_rate = plunge::resolve_plunge_at(&plunge_basis, unlifted_feed(s), s.speed.rpm)
        .map_or(plunge, |(value, _)| value);
    Entry {
        ramp,
        plunge_basis,
        plunge_rate,
        material_plunge_row,
    }
}

/// Step 9b: the rubbing-floor warning on the final feed.
pub(super) fn step9b_rubbing_floor(s: &mut CalcState) {
    // --- Step 9b: Rubbing-floor warning on the final feed ---
    //
    // Below the chip-formation threshold the edge rubs instead of cutting,
    // heats, and can burn the work. Two paths can put the commanded advance
    // per tooth (`feed / (rpm * flutes)`) under it:
    //   1. A vendor row scaled down for a hard species or a small tool.
    //   2. The feed de-rates (safety factor, L/D, power) on a target that is
    //      already low.
    //
    // Ruling R4 WP2a (2026-09-23): this step WARNS and does not lift. Until
    // then it raised the feed to the floor. The floor constant is a repo
    // rule with no source, and the lift contradicted the vendor bands below
    // 0.025 mm/tooth (EVIDENCE 4.1-26). The recipe now ships its computed
    // feed and `FeedsWarning::ChiploadBelowRubbingFloor` names the levers.
    //
    // The `> 0.0` guard keeps a zero-feed regression (for example on the
    // RPM-only LUT formula path) visible. The warning does not mask it.
    //
    // The floor is `rubbing_floor(band)` = `min(0.025, band min)`, or
    // `min(0.025, band max)` when the band has no minimum (ruling R4 Q9).
    // With no band, a printed point gives `min(0.025, point)`
    // (`rubbing_floor_at`, A2).
    // The band is the recipe resolver's band when it published one, and
    // otherwise the envelope resolver's band (P1, 2026-08-22): the band that
    // the post-sim gate also uses.
    //
    // ── The chipload band (feeds contract P11b, L6: chip first) ───────
    //
    // The band that this function returns is the printed band (after the
    // size and hardness claims), at every depth. Until P11b it was de-rated
    // at the shipped depth by the vendor depth table
    // (`geometry::doc_derating_scale`); the table is now a limit of the
    // envelope ("vendor depth ceiling (width not printed)"), not a scale on
    // the chip. The band holds the PEAK chip (L1): the feed above is sized
    // so that the peak chip is the band midpoint.
    //
    // P11b2: the post-sim chipload gate (`tool_load::chipload`) and the feed
    // modulator read this printed band too, and judge the peak chip through
    // the same law (`geometry::RadialThinning`).
    let fpt_divisor = s.speed.rpm * s.input.flute_count as f64;
    if fpt_divisor > 0.0 {
        let commanded_fpt = s.feed.feed / fpt_divisor;
        let floor_band = s.chip.band.or(s.chip.floor_band_fallback);
        // The recipe row's point when it has one; else the envelope row's
        // point. A band outranks a point (`rubbing_floor_at`).
        let (floor, source) = s.chip.rubbing_floor();
        // P16 (D8): the last power rung can ship the floor feed itself; the
        // relative margin keeps a rounding step under the floor from
        // warning.
        if commanded_fpt > 0.0 && commanded_fpt < floor * (1.0 - 1e-9) {
            s.warnings.push(FeedsWarning::ChiploadBelowRubbingFloor {
                commanded: commanded_fpt,
                floor,
                source,
                band_min: floor_band.map(|b| b.min_mm_per_tooth).filter(|m| *m > 0.0),
                band_max: floor_band.map(|b| b.max_mm_per_tooth),
            });
        }
    }
}

/// Step 9c: a drill cycle keeps its feed in the material plunge envelope
/// and at or below the machine Z axis rate, and its plunge is its feed.
pub(super) fn step9c_drill(s: &mut CalcState, entry: &mut Entry) {
    let input = s.input;
    let machine = input.machine;
    let d = input.tool_diameter;
    // --- Step 9c: Drill cycles — envelope sanity + "feed IS plunge" ---
    //
    // A drill cycle has exactly one feed: the plunge. Corrections 1 and 2
    // close the 2026-06-10 defect-class findings (F1, see
    // `planning/DEFECT_CLASS_CLEANUP_2026-06-10.md`); correction 3 is
    // P20b:
    //
    // 1. Clamp the chipload-derived feed into the material plunge-feed
    //    envelope (`Material::drill_plunge_feed_envelope_per_mm`, units
    //    feed/Ø per minute). Never silently serve a recipe in the
    //    rubbing band below the envelope or the breakage band above it.
    //    (The rubbing floor of Step 9b only warns since R4 WP2a; this
    //    drill envelope still clamps.)
    //    The COMMANDED cutting-feed ceiling still wins over the envelope
    //    floor: a machine that can't reach the floor gets the honest
    //    conflict via the warning rather than an unreachable feed. This
    //    clamp runs after Step 9, so it works on the COMMANDED axis and
    //    the cap it reads is `machine.cutting_feed_ceiling_mm_min()`, the
    //    Step 7 ceiling. T-18 (2026-09-18) replaced
    //    `machine.max_feed_mm_min × safety_factor` here, which is a
    //    fraction of the gantry TRAVEL rate and a different quantity.
    //    Ruling R4 removed the factor, so the commanded and the cutting
    //    ceilings are one number.
    // 2. Alias `plunge_rate` to the final drill feed. Drill op configs
    //    alias `set_feed_rate` / `set_plunge_rate` onto one field
    //    ("feed IS plunge"), and `apply_feeds_subset` writes feed then
    //    plunge — pre-fix, the milling plunge baseline from Step 8
    //    clobbered the drill-tuned feed (the RPM and the chipload of
    //    Steps 1-2), landing every suggested drill at the milling
    //    plunge value instead. Making the result self-consistent here
    //    keeps any write order safe.
    // 3. P20b: the machine Z axis rate (GRBL `$112`) caps the drill feed
    //    (`feeds::vertical::VerticalRateLimit`, the one vertical-rate
    //    limit; a drill cycle has no G6 ramp basis, so only the Z axis rate
    //    binds). Above it the machine runs slower than the card says. A
    //    machine with no per-axis rates has no Z cap.
    //
    // When the envelope ceiling or the Z axis rate lowers the feed, the RPM
    // follows down once, over the combined clamp, so that the commanded
    // chipload holds and does not thin toward rubbing. A capped drill that
    // keeps a high RPM takes a thinner bite per revolution. The machine
    // minimum RPM (`machine.clamp_rpm`) is the only lower bound: P20a
    // (2026-11-08) removed the drill RPM tier, which had no source. The
    // envelope ceilings are the largest printed Ramp Down per mm in the G6
    // rule range (the Amana Spektra 3 mm row), so a claimed G6 cell at the
    // chart chip and RPM is at or just under the ceiling at 3 mm and below
    // it at a larger diameter. A higher RPM (`SpindleStrategy::MaxSpeed`), a
    // hardness transfer that lifts the chip, or a formula preview can bind
    // it.
    if input.operation == OperationFamily::Drill {
        let (env_lo_per_mm, env_hi_per_mm) = input.material.drill_plunge_feed_envelope_per_mm();
        let (env_lo, env_hi) = (env_lo_per_mm * d, env_hi_per_mm * d);
        let requested = s.feed.feed;
        let mut feed = requested;
        if feed < env_lo || feed > env_hi {
            let commanded_cut_ceiling = machine.cutting_feed_ceiling_mm_min();
            let clamped = feed.clamp(env_lo, env_hi).min(commanded_cut_ceiling);
            s.warnings.push(FeedsWarning::DrillFeedClampedToEnvelope {
                requested,
                actual: clamped,
                envelope_lo: env_lo,
                envelope_hi: env_hi,
            });
            feed = clamped;
            s.feed.basis = Basis::derived("drill feed envelope", vec![s.feed.basis.clone()]);
        }
        let rpm = s.speed.rpm;
        let vertical = entry.plunge_basis.vertical_limit().bound(feed, rpm);
        let z_capped = match vertical.binding() {
            VerticalBinding::ZAxisRate => true,
            VerticalBinding::Candidate | VerticalBinding::AxialChip => false,
        };
        let before_z = feed;
        if z_capped {
            feed = vertical.mm_min();
            s.feed.basis = Basis::derived(
                "machine Z axis rate",
                vec![
                    Basis::Machine(sourced::MachineField::ZAxisRate),
                    s.feed.basis.clone(),
                ],
            );
        }
        let flutes = input.flute_count as f64;
        if feed < requested && rpm > 0.0 && flutes > 0.0 {
            let kept_fpt = requested / (rpm * flutes);
            if kept_fpt > 0.0 {
                let target_rpm = machine.clamp_rpm(feed / (kept_fpt * flutes));
                if target_rpm < rpm {
                    s.speed.spindle_scale *= target_rpm / rpm;
                    s.speed.rpm = target_rpm;
                }
            }
        }
        if z_capped {
            s.warnings.push(FeedsWarning::DrillFeedAtZAxisRate {
                requested: before_z,
                z_rate_mm_min: feed,
                rpm_from: rpm,
                rpm_to: s.speed.rpm,
            });
        }
        s.feed.feed = feed;
        entry.plunge_rate = s.feed.feed;
    }
}

/// The result: the plunge, ramp and power bases at the final feed, the
/// published power, the derates and the bases of the point.
pub(super) fn result(s: CalcState, entry: Entry) -> FeedsResult {
    let CalcState {
        input,
        warnings,
        speed,
        chip,
        cut,
        feed,
    } = s;
    let machine = input.machine;
    let Entry {
        ramp,
        plunge_basis,
        plunge_rate,
        material_plunge_row,
    } = entry;
    // P5 (K2): the plunge basis at the final feed, the ramp basis, and the
    // power basis.
    let plunge_value_basis = plunge_value_basis(
        &plunge_basis,
        UnliftedFeed::at_cut(feed.feed, feed.thinning, cut.ae, feed.effective_d),
        speed.rpm,
        &feed.basis,
        &material_plunge_row,
    );
    let ramp_value_basis = match &ramp {
        RampBasis::Sourced { drill, .. } => Basis::Claim(sourced::ClaimRef {
            kind: sourced::ClaimKind::DrillG6,
            row: sourced::RowRef(drill.side_row.clone()),
            rule: "g6_ramp".to_owned(),
        }),
        RampBasis::NoChip { .. } => {
            Basis::derived("ramp at the plunge", vec![plunge_value_basis.clone()])
        }
    };

    // Final power at actual feed, through the same helpers as Step 6, so
    // Suggest's reported power matches the Sim verdict's prediction.
    // K7 change 3: the published power is the worst case of the set, and
    // the result names the case.
    // P20d (P18 item 3b): `None` when not modelled: no force line, a depth
    // known after the simulation (P16, `FeedsInput::power_force_line`), or
    // a plunge of a tip with no axial section. A drill cycle reads the axial
    // circle (advisory: Step 6 did not act on it).
    let line = input.power_force_line();
    let (power_case, actual_power, section_area) = if input.is_axial_plunge() {
        let terms = line.and_then(|l| crate::feeds::axial_power_terms(input, l, speed.rpm));
        let area = crate::feeds::section::EngagedSection::axial_plunge(
            input.tool_geometry,
            input.tool_diameter,
        )
        .ok()
        .map(|s| s.area_mm2());
        (
            engagement::PowerCase::AxialPlunge,
            terms.map(|t| t.kw_at_feed(feed.feed)),
            area,
        )
    } else {
        let section = engagement::Engagement::from_cut(
            crate::feeds::envelope::CutPoint {
                ap_mm: cut.ap,
                ae_mm: cut.ae,
            },
            input.tool_geometry,
            feed.effective_d,
            None,
        )
        .section_mm2();
        match line {
            Some(line) => {
                let (case, kw, _) = worst_case_power(
                    input,
                    line,
                    cut.ap,
                    cut.ae,
                    feed.effective_d,
                    speed.rpm,
                    feed.feed,
                );
                (case, Some(kw), Some(section))
            }
            None => (engagement::PowerCase::Nominal, None, Some(section)),
        }
    };
    // P16 (item 2b): no removal rate where the depth is known after the
    // simulation; the depth value there is the tool diameter, not a cut.
    // P20d: the removal rate is the engaged section x the feed.
    let mrr = if input.depth_after_simulation() {
        Err(crate::feeds::operating_point::PowerUnmodeled::DepthAfterSimulation)
    } else {
        section_area
            .map(|a| a * feed.feed)
            .ok_or(crate::feeds::operating_point::PowerUnmodeled::AxialTipNotModelled)
    };

    let cl = &machine.chip_load;
    let formula = if matches!(
        chip.chipload_source,
        ChiploadSource::FormulaFallback | ChiploadSource::EdgeRadiusFloor
    ) {
        Some(FormulaBreakdown {
            k0: cl.k0,
            p: cl.p,
            q: cl.q,
            diameter_mm: input.tool_diameter,
            feed_scale_factor: chip.feed_scale,
            result_mm_tooth: chip.formula_chipload,
        })
    } else {
        None
    };

    let derates = FeedsDerates {
        target_chip_load_mm: chip.chip_load,
        formula,
        radial_chip_thinning: feed.radial_lift,
        observed_axial_chip_thinning: feed.axial_thinning,
        ld_overhang: feed.ld_factor,
        power_limit: feed.power_factor,
        feed_clamp: feed.feed_clamp_factor,
        spindle_scale: speed.spindle_scale,
        spindle_scale_reason: speed.spindle_scale_reason,
    };

    FeedsResult {
        rpm: speed.rpm,
        chip_load_mm: chip.chip_load,
        feed_rate_mm_min: feed.feed,
        plunge_rate_mm_min: plunge_rate,
        plunge: plunge_basis,
        ramp,
        axial_depth_mm: cut.ap,
        radial_width_mm: cut.ae,
        power_kw: actual_power,
        power_case,
        // F-2: the gate's ceiling, not the raw spindle curve — this is
        // the denominator `power_kw` (evaluated at the FINAL feed) is
        // rendered against. See the Step 6 axis note above. It is read at
        // the FINAL RPM: Step 7 (R4 Q10) can lower the RPM after Step 6,
        // and a VFD's rated power falls with it, so the Step 6 value would
        // overstate the headroom (G6 Spektra load, 2026-09-25: Ø12 slot on
        // the Shapeoko VFD, 1.125 kW at 18 000 rpm against 1.079 kW at the
        // shipped 17 265 rpm).
        available_power_kw: machine.power_at_rpm(speed.rpm),
        power_limited: feed.power_limited,
        mrr_mm3_min: mrr,
        warnings,
        vendor_source: chip.vendor_source,
        support: chip.support,
        chipload_source: chip.chipload_source,
        chipload_bounds: chip.band,
        chipload_point_mm: chip.point,
        matched_lut_row: chip.matched_lut_row,
        effective_diameter_mm: feed.effective_d.mm(),
        derates,
        bases: PointBases {
            power: Basis::derived(
                "two-term power model",
                vec![
                    feed.basis.clone(),
                    cut.ap_basis.clone(),
                    cut.ae_basis.clone(),
                    speed.basis.clone(),
                ],
            ),
            rpm: speed.basis,
            chip: chip.basis,
            feed: feed.basis,
            plunge: plunge_value_basis,
            ramp: ramp_value_basis,
            flute_count: super::feed::flute_count_basis(input),
            ap: cut.ap_basis,
            ae: cut.ae_basis,
        },
        // `calculate_with` adds the step records.
        steps: Vec::new(),
    }
}
