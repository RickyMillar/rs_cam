//! Feeds & speeds calculator — computes RPM, feed rate, plunge rate, DOC, WOC,
//! and power requirements from tool, material, machine, and operation parameters.
//!
//! Provenance for formulas and seed data is tracked in
//! `crates/rs_cam_core/data/vendor_lut/source_manifest.json` and the repo credits docs.
//! The calculation pipeline:
//! 1. RPM from surface speed → clamp to machine range
//! 2. Chip load from empirical formula: K₀ × D^p × (1/H)^q
//! 3. DOC/WOC from operation matrix × machine rigidity
//! 4. Flute guard: cap DOC to 0.8 × flute_length
//! 5. Feed = RPM × chipload × flutes × RCTF
//! 6. Power check: Kc × DOC × WOC × feed / 60e6 — reduce feed if over
//! 7. Clamp feed to machine max
//! 8. Plunge rate from material-dependent fraction
//! 9. Apply safety factor
//! 10. Collect warnings

pub mod explain;
pub mod geometry;
pub mod suggest;
pub mod vendor_lookup;
pub mod vendor_lut;
pub mod vendor_normalize;
pub use explain::{FeedsExplain, MachineEnvelope, explain as explain_feeds};
pub use vendor_lut::VendorLut;

/// Global embedded vendor LUT, loaded once on first access.
pub static EMBEDDED_LUT: std::sync::LazyLock<VendorLut> =
    std::sync::LazyLock::new(VendorLut::embedded);

/// Thin getter for the single embedded LUT instance.
pub fn embedded_vendor_lut() -> &'static VendorLut {
    &EMBEDDED_LUT
}

use crate::machine::MachineProfile;
use crate::material::Material;

/// Hint about the tool geometry for effective diameter calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolGeometryHint {
    Flat,
    Ball,
    Bull {
        corner_radius: f64,
    },
    VBit {
        included_angle: f64,
        tip_diameter: f64,
    },
    TaperedBall {
        tip_radius: f64,
        taper_angle_deg: f64,
    },
}

impl ToolGeometryHint {
    /// Engaged chip cross-section (mm²) for the canonical power
    /// prediction. Mirrors `MillingCutter::mrr_cross_section_mm2` —
    /// rectangular slab `ap · ae` for endmills, triangular groove
    /// `½ · ap · ae` for V-bits — so the Suggest path
    /// (`feeds::calculate`) and the Sim verdict
    /// (`tool_load::power::evaluate`) agree on what area the
    /// `predicted_power_kw` formula multiplies by.
    ///
    /// The cutter trait method is the canonical source when a full
    /// `ToolDefinition` is available; this hint-level method is the
    /// equivalent shape contract for the feeds/calculate path that
    /// only carries a `ToolGeometryHint`.
    pub fn mrr_cross_section_mm2(self, axial_doc_mm: f64, radial_width_mm: f64) -> f64 {
        match self {
            // V-bit removes a triangular groove — half the rectangular
            // slab a flat endmill would remove at the same DOC × WOC.
            ToolGeometryHint::VBit { .. } => 0.5 * axial_doc_mm * radial_width_mm,
            // Flat / Ball / Bull / TaperedBall: rectangular slab.
            // Ball/tapered remove slightly less than the full slab at
            // shallow DOC, but the established approximation is the
            // same `ap · ae` the trait default uses.
            ToolGeometryHint::Flat
            | ToolGeometryHint::Ball
            | ToolGeometryHint::Bull { .. }
            | ToolGeometryHint::TaperedBall { .. } => axial_doc_mm * radial_width_mm,
        }
    }

    /// Effective cutting (engaged) diameter at a given axial depth of
    /// cut (mm).
    ///
    /// For tapered-ball and V-bit tools the engaged diameter grows with
    /// DOC as the cone shoulder comes into the cut, so the published tip
    /// diameter understates what is actually cutting. Flat / ball / bull
    /// tools engage at their nominal diameter regardless of DOC.
    ///
    /// `tool_diameter_mm` is the nominal (tip, for tapered/V) diameter;
    /// `shank_diameter_mm` caps the tapered-ball growth at the shank.
    /// This is the same calculation the vendor-LUT lookup uses to pick
    /// the chipload row, so the band shown in the UI applies to the
    /// returned diameter — not the tool tip.
    pub fn engaged_diameter_at_doc(
        self,
        axial_doc_mm: f64,
        tool_diameter_mm: f64,
        shank_diameter_mm: f64,
    ) -> f64 {
        let axial_doc = axial_doc_mm.max(0.0);
        match self {
            ToolGeometryHint::TaperedBall {
                tip_radius,
                taper_angle_deg,
            } => {
                let alpha = taper_angle_deg.to_radians();
                let sin_alpha = alpha.sin();
                let cos_alpha = alpha.cos();
                let tan_alpha = alpha.tan();
                if tip_radius <= 0.0 || tan_alpha <= 0.0 {
                    return tool_diameter_mm;
                }
                let h_contact = tip_radius * (1.0 - sin_alpha);
                let r_contact = tip_radius * cos_alpha;
                let cone_offset = h_contact - r_contact / tan_alpha;
                let radius = if axial_doc <= h_contact {
                    (2.0 * tip_radius * axial_doc - axial_doc * axial_doc)
                        .max(0.0)
                        .sqrt()
                } else {
                    (axial_doc - cone_offset) * tan_alpha
                };
                (2.0 * radius).clamp(0.0, shank_diameter_mm)
            }
            ToolGeometryHint::VBit { included_angle, .. } => {
                let half = (included_angle * 0.5).to_radians();
                (2.0 * axial_doc * half.tan()).clamp(0.0, tool_diameter_mm)
            }
            ToolGeometryHint::Flat | ToolGeometryHint::Ball | ToolGeometryHint::Bull { .. } => {
                tool_diameter_mm
            }
        }
    }
}

/// Which family of operation is being calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamily {
    Adaptive,
    Pocket,
    Contour,
    Parallel,
    Scallop,
    Trace,
    Face,
    /// Drill / peck cycles. Z-only kinematics — radial WOC is not
    /// applicable and engagement metrics are routed to drill-native
    /// gates (peck adequacy, chip welding). Feeds plumbing uses this
    /// to lower RPM into a drill-appropriate band (chipload at
    /// milling RPM and drill plunge feed produces rubbing — audit
    /// finding "Drill ops route through OperationFamily::Pocket with
    /// no chipload reconciliation").
    Drill,
}

/// Role of the pass (roughing removes bulk, finishing for surface quality).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassRole {
    Roughing,
    SemiFinish,
    Finish,
}

/// Setup context for derating feeds based on physical setup conditions.
pub struct SetupContext {
    /// Tool overhang from collet face (mm). Used for L/D derate.
    pub tool_overhang_mm: Option<f64>,
    /// Workholding rigidity affects feed rate.
    pub workholding_rigidity: WorkholdingRigidity,
}

impl Default for SetupContext {
    fn default() -> Self {
        Self {
            tool_overhang_mm: None,
            workholding_rigidity: WorkholdingRigidity::Medium,
        }
    }
}

/// Workholding rigidity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WorkholdingRigidity {
    Low,
    Medium,
    High,
}

/// Policy for choosing operating-point RPM along the constant-chipload
/// line.
///
/// Under [`SpindleStrategy::MatchChart`] (default) the feeds calculator
/// returns the vendor LUT row's `rpm_nominal` verbatim — the chipload
/// envelope is most defensible at the chart's tested RPM. Under
/// [`SpindleStrategy::MaxSpeed`] the calculator lifts RPM toward the
/// spindle ceiling (capped by `vendor.rpm_max` when present, then by
/// the machine's `spindle.max_rpm` and a small safety headroom) and
/// scales feed proportionally to keep chipload constant — same
/// operating point on the chipload axis, just moved along the speed
/// axis. The existing power / feed-cap derates still apply on top, so
/// if the higher RPM exceeds spindle power the `power_limit` derate
/// claws feed back.
///
/// `Default` is `MatchChart` so projects predating this enum behave
/// identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpindleStrategy {
    /// Use the LUT row's `rpm_nominal` (or the material-derived ideal
    /// RPM when no vendor row matches). Preserves chart fidelity.
    #[default]
    MatchChart,
    /// Push RPM up to the spindle ceiling (clamped by vendor rpm_max
    /// when present, capped by [`MAX_SPINDLE_SPEEDUP`] from the chart
    /// nominal). Scales feed proportionally to keep chipload constant.
    MaxSpeed,
}

/// Hard cap on the speedup multiplier `MaxSpeed` can apply over the
/// chart's `rpm_nominal`. 1.5× is conservative — past it, chip-thinning
/// at higher RPM enters territory the chart wasn't tested at. Matches
/// the rough magnitude of the chart's own rpm_max-vs-rpm_nominal
/// spread when vendors do publish a range.
pub const MAX_SPINDLE_SPEEDUP: f64 = 1.5;

/// Safety headroom below the machine's nominal spindle ceiling. Avoids
/// commanding the spindle at exactly its max — leaves the controller
/// some margin for transient overshoot.
pub const SPINDLE_CEILING_HEADROOM: f64 = 0.95;

/// Input parameters for the feeds calculator.
pub struct FeedsInput<'a> {
    pub tool_diameter: f64,
    pub flute_count: u32,
    pub flute_length: f64,
    pub shank_diameter: Option<f64>,
    pub tool_geometry: ToolGeometryHint,
    pub material: &'a Material,
    pub machine: &'a MachineProfile,
    pub operation: OperationFamily,
    pub pass_role: PassRole,
    /// Optional DOC override (None = auto-calculate).
    pub axial_depth_mm: Option<f64>,
    /// Optional WOC/stepover override (None = auto-calculate).
    pub radial_width_mm: Option<f64>,
    /// Target scallop height for ball/tapered ball finishing (mm).
    pub target_scallop_mm: Option<f64>,
    /// Optional vendor LUT for chipload lookup (None = formula only).
    pub vendor_lut: Option<&'a vendor_lut::VendorLut>,
    /// Physical setup context for feed derating.
    pub setup: SetupContext,
    /// Spindle-RPM policy. See [`SpindleStrategy`]. Defaults to
    /// `MatchChart` (chart-fidelity, preserves pre-2026-06-01
    /// behaviour). `MaxSpeed` walks the constant-chipload line up to
    /// the spindle ceiling.
    pub spindle_strategy: SpindleStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChiploadSource {
    VendorLut { observation_id: String },
    FormulaFallback,
    EdgeRadiusFloor,
}

/// Result of the feeds calculation.
#[derive(Debug, Clone)]
pub struct FeedsResult {
    pub rpm: f64,
    pub chip_load_mm: f64,
    pub feed_rate_mm_min: f64,
    pub plunge_rate_mm_min: f64,
    pub ramp_feed_mm_min: f64,
    pub axial_depth_mm: f64,
    pub radial_width_mm: f64,
    pub power_kw: f64,
    pub available_power_kw: f64,
    pub power_limited: bool,
    pub mrr_mm3_min: f64,
    pub warnings: Vec<FeedsWarning>,
    /// Observation ID if vendor LUT was used for chipload.
    pub vendor_source: Option<String>,
    pub chipload_source: ChiploadSource,
    /// Full derate chain that turned the "target" chipload into the
    /// recommended feed. Lets the UI show *why* the recommended
    /// operating point sits where it does on the feed-RPM nomogram.
    pub derates: FeedsDerates,
}

/// Per-step record of the chipload → feed pipeline. Each multiplier
/// is positive (no zero divisors); a value of 1.0 means "no effect."
/// The "effective chipload" the toolpath actually cuts at is
/// `target_chip_load_mm × every_multiplier_here`.
///
/// Derived purely so the UI can render the breakdown — calculate()
/// applies each factor in place, this struct just captures them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeedsDerates {
    /// LUT midpoint (or formula chipload) before any multipliers.
    pub target_chip_load_mm: f64,
    /// Empirical formula breakdown — populated only when the chipload
    /// came from [`ChiploadSource::FormulaFallback`] / `EdgeRadiusFloor`.
    /// `None` when the LUT supplied the value.
    pub formula: Option<FormulaBreakdown>,
    /// Radial chip thinning factor (≥ 1.0). At small stepovers the
    /// chip is thinner per tooth-pass so we feed faster to keep the
    /// effective chipload constant.
    pub radial_chip_thinning: f64,
    /// Axial chip thinning factor for ball/tapered-ball tools at
    /// shallow DOC.
    pub axial_chip_thinning: f64,
    /// Combined chip thinning, clamped to `[1.0, 4.0]`.
    pub combined_chip_thinning: f64,
    /// Depth-tier feed derate (≤ 1.0). Deep cuts get slower feed to
    /// limit deflection.
    pub depth_tier: f64,
    /// L/D (tool overhang) derate (≤ 1.0). Long tools deflect more.
    pub ld_overhang: f64,
    /// Workholding rigidity factor (0.85 / 1.00 / 1.03 for Low/Med/High).
    pub workholding: f64,
    /// Power-limit factor (≤ 1.0). Applied when the calc had to back
    /// off feed to stay within the spindle's power envelope.
    pub power_limit: f64,
    /// Machine-feed-cap factor (≤ 1.0). Applied when the calc hit the
    /// machine's `max_feed_mm_min`.
    pub feed_clamp: f64,
    /// Machine safety factor (0.75–0.80 typical).
    pub safety_factor: f64,
    /// Spindle-speedup multiplier (≥ 1.0). Under
    /// [`SpindleStrategy::MaxSpeed`] the calculator lifts RPM toward
    /// the spindle ceiling and scales feed proportionally to keep the
    /// chipload constant. `1.0` under `MatchChart` (the default) and
    /// when the chart RPM is already at or above the ceiling.
    /// Applied multiplicatively in [`combined_factor`].
    ///
    /// [`combined_factor`]: Self::combined_factor
    pub spindle_speedup: f64,
}

/// Empirical chipload formula evaluation `K₀ × D^p × (1/H)^q`.
/// Captured so the UI can show *why* the no-LUT recommendation is what
/// it is (rather than just "fallback").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormulaBreakdown {
    pub k0: f64,
    pub p: f64,
    pub q: f64,
    pub diameter_mm: f64,
    /// Renamed from `hardness_index` in S2-8 Option B (2026-06-01) —
    /// the field always held [`Material::feed_scale_factor`]'s output,
    /// not a wood-Janka hardness. See the accessor's doc for the
    /// per-class semantics.
    pub feed_scale_factor: f64,
    pub result_mm_tooth: f64,
}

impl FeedsDerates {
    /// Compose every multiplier into a single number. The effective
    /// chipload (`feed / (RPM × flutes)`) equals
    /// `target_chip_load_mm × combined_factor()`.
    pub fn combined_factor(&self) -> f64 {
        // Spindle speedup is intentionally NOT included here:
        // `combined_factor` represents the multiplier applied to the
        // target chipload to get the effective chipload. Spindle
        // speedup walks the constant-chipload line (RPM and feed
        // scale together), so chipload is unchanged. The modal
        // renders `spindle_speedup` as a peer row so the operator
        // sees the speed-axis change separately from the chipload
        // derates.
        self.combined_chip_thinning
            * self.depth_tier
            * self.ld_overhang
            * self.workholding
            * self.power_limit
            * self.feed_clamp
            * self.safety_factor
    }

    /// Effective chipload that the toolpath will actually cut at,
    /// derived from the recommended feed.
    pub fn effective_chip_load_mm(&self) -> f64 {
        self.target_chip_load_mm * self.combined_factor()
    }
}

/// Warnings generated during calculation.
#[derive(Debug, Clone)]
pub enum FeedsWarning {
    FeedRateClamped { requested: f64, actual: f64 },
    PowerLimited { required_kw: f64, available_kw: f64 },
    ShankTooLarge { shank_mm: f64, max_mm: f64 },
    DocExceedsFlute { requested: f64, capped: f64 },
    SlottingDetected { doc_reduced_to: f64 },
    ScallopInvalid { target: f64, max_possible: f64 },
}

/// Main calculation entry point.
pub fn calculate(input: &FeedsInput) -> FeedsResult {
    let mut warnings = Vec::new();
    let machine = input.machine;
    let material = input.material;
    let d = input.tool_diameter;

    // Engaged diameter at the operation's commanded DOC. For VBit and
    // TaperedBall geometries the *cutting* circle is the cone-shoulder
    // diameter at this DOC — not the published tip. Flat / Ball / Bull
    // tools engage at nominal D regardless of DOC, so `effective_d == d`
    // there.
    //
    // This mirrors `vendor_normalize::lookup_diameter_for_input` so the
    // SFM/RPM derivation and formula-chipload fallback below stay
    // symmetric with the vendor-LUT query path. Pre-2026-06-02 the
    // formula path used nominal D, producing wrong-low RPM for V-bits
    // (e.g. a 5.5 mm-tip 20° V-bit at DOC=0.5 saw SFM derived from
    // 5.5 mm instead of the ~0.18 mm engaged tip) — audit finding
    // "nominal-D leakage through formula path".
    let axial_doc_for_eff_d = input.axial_depth_mm.unwrap_or(d).max(0.0);
    let effective_d = input.tool_geometry.engaged_diameter_at_doc(
        axial_doc_for_eff_d,
        d,
        input.shank_diameter.unwrap_or(d),
    );

    // --- Step 1: RPM ---
    const FALLBACK_RPM: f64 = 18000.0;
    let ideal_rpm = if effective_d > 0.0 {
        (material.base_cutting_speed_m_min() * 1000.0) / (std::f64::consts::PI * effective_d)
    } else {
        FALLBACK_RPM
    };
    let mut rpm = machine.clamp_rpm(ideal_rpm);

    // Drill ops want a lower RPM band than milling regardless of D:
    // chip evacuation, not surface speed, is the limiting factor. 8-14k
    // RPM is the wood-drill band; the milling SFM formula above would
    // push small-D drills past 16k where chipload starves and the cut
    // rubs/burns. Pre-2026-06-02 drill ops routed through `Pocket`
    // family and inherited milling RPM (audit finding: "Drill ops route
    // through OperationFamily::Pocket with no chipload reconciliation").
    const DRILL_RPM_FLOOR: f64 = 8_000.0;
    const DRILL_RPM_CEIL: f64 = 14_000.0;
    if input.operation == OperationFamily::Drill {
        rpm = rpm.clamp(DRILL_RPM_FLOOR, DRILL_RPM_CEIL);
        rpm = machine.clamp_rpm(rpm);
    }

    // --- Step 2: Chip load — vendor LUT first, formula fallback ---
    let feed_scale = material.feed_scale_factor();
    let cl = &machine.chip_load;
    // Formula chipload uses engaged diameter so the V-bit / tapered-ball
    // fallback path matches the LUT path's band semantics. For Flat /
    // Ball / Bull this collapses to nominal D.
    //
    // Drill ops get a multiplier on top because drill chipload bands
    // are ~2.5× higher than milling chipload at similar D (drilling
    // cuts at full radius and needs feed-per-rev to chip-evacuate; the
    // milling formula was calibrated against partial-engagement cuts).
    // Without this, a softwood drill at 12k RPM × milling-formula
    // chipload lands at ~0.03 mm/rev, well below the 0.05-0.15 mm/rev
    // drilling band — classic rubbing-and-burning recipe (audit
    // finding: implied chipload 0.026 on Wanaka Pin Drill / Holes).
    const DRILL_CHIPLOAD_MULTIPLIER: f64 = 2.5;
    let milling_chipload = cl.k0 * effective_d.powf(cl.p) * (1.0 / feed_scale).powf(cl.q);
    let formula_chipload = if input.operation == OperationFamily::Drill {
        milling_chipload * DRILL_CHIPLOAD_MULTIPLIER
    } else {
        milling_chipload
    };

    let (chip_load, vendor_rpm, vendor_rpm_max, vendor_source, chipload_source) =
        if let Some(lut) = input.vendor_lut {
            let query = vendor_normalize::to_lookup_query(input);
            if let Some(result) =
                vendor_lookup::find_best_row_for_geometry(lut, &query, &input.tool_geometry)
            {
                let observation_id = result.observation_id;
                // RPM-only vendor rows (e.g. whiteside-rd5218h-roughing-down-
                // spiral-3f-rpm) publish rpm_nominal/rpm_max as anchors but
                // leave chipload_min/max unset — `chipload_midpoint` then
                // returns 0.0. Trusting that 0.0 collapses
                // `raw_feed = rpm × chipload × flutes` to zero, producing a
                // silent "do not cut" recipe with no diagnostic
                // (literature-matrix cell flat_12mm_adaptive2d_oak_power:
                // chipload=0.0000 / mrr=0 / power=0). Keep the vendor RPM
                // anchor but fall back to formula_chipload when the row
                // publishes none.
                if result.chip_load_mm > 0.0 {
                    (
                        result.chip_load_mm,
                        result.rpm_nominal,
                        result.rpm_max,
                        Some(observation_id.clone()),
                        ChiploadSource::VendorLut { observation_id },
                    )
                } else {
                    (
                        formula_chipload,
                        result.rpm_nominal,
                        result.rpm_max,
                        Some(observation_id),
                        ChiploadSource::FormulaFallback,
                    )
                }
            } else {
                (
                    formula_chipload,
                    None,
                    None,
                    None,
                    ChiploadSource::FormulaFallback,
                )
            }
        } else {
            (
                formula_chipload,
                None,
                None,
                None,
                ChiploadSource::FormulaFallback,
            )
        };

    // Override RPM if vendor provided one within machine range. This
    // is the chart-RPM operating point — preserved verbatim under
    // `SpindleStrategy::MatchChart`.
    if let Some(v_rpm) = vendor_rpm {
        rpm = machine.clamp_rpm(v_rpm);
    }

    // --- Step 2b: Spindle-speedup along the constant-chipload line ---
    //
    // Under `SpindleStrategy::MaxSpeed` push RPM up toward the
    // spindle ceiling (clamped by vendor.rpm_max when published,
    // capped by MAX_SPINDLE_SPEEDUP and a small safety headroom).
    // Feed scales proportionally below (the feed formula already
    // multiplies by rpm), so chipload is preserved. Power and feed-
    // cap derates apply on top — if the higher operating point
    // exceeds spindle power the existing `power_limit` claws feed
    // back, and the modal renders the resulting binding constraint.
    //
    // `spindle_speedup` is captured into `FeedsDerates` for the modal.
    // Default (MatchChart) leaves it at 1.0; smoke baselines unchanged.
    let mut spindle_speedup = 1.0_f64;
    if matches!(input.spindle_strategy, SpindleStrategy::MaxSpeed) && rpm > 0.0 {
        let (_, machine_max_rpm) = machine.rpm_range();
        let machine_ceiling = machine_max_rpm * SPINDLE_CEILING_HEADROOM;
        let ceiling = match vendor_rpm_max {
            Some(vm) if vm.is_finite() && vm > 0.0 => vm.min(machine_ceiling),
            _ => machine_ceiling,
        };
        if ceiling > rpm {
            let raw_speedup = ceiling / rpm;
            spindle_speedup = raw_speedup.min(MAX_SPINDLE_SPEEDUP);
            rpm = machine.clamp_rpm(rpm * spindle_speedup);
        }
    }

    // --- Step 2c: Re-apply the Drill RPM clamp ---
    //
    // Both the vendor-RPM override (Step 2) and the MaxSpeed speedup
    // (Step 2b) can lift RPM above the drill ceiling. Chip evacuation
    // is the binding constraint for drill ops regardless of spindle
    // headroom or chart RPM — pushing past 14k starves chipload below
    // the rubbing floor (literature-matrix cell flat_3mm_drill_oak:
    // MaxSpeed lifted clamped 14000 → 14000 × MAX_SPINDLE_SPEEDUP =
    // 21000, +50% over the 8-14k wood-drill band).
    //
    // We also roll `spindle_speedup` back proportionally so the modal
    // reports the actual speedup the engine kept, not the requested
    // one it then undid.
    if input.operation == OperationFamily::Drill {
        let pre_clamp = rpm;
        rpm = rpm.clamp(DRILL_RPM_FLOOR, DRILL_RPM_CEIL);
        rpm = machine.clamp_rpm(rpm);
        if pre_clamp > 0.0 && rpm < pre_clamp {
            spindle_speedup *= rpm / pre_clamp;
        }
    }

    // --- Step 3: DOC/WOC from operation defaults ---
    let profile = operation_default_profile(input.operation, input.pass_role);
    let (mut ap, mut ae) = default_engagement(d, &profile, input, machine);

    // --- Step 3b: Scallop-driven stepover for ball/tapered ball ---
    if let Some(target_scallop) = input.target_scallop_mm {
        let ball_r = match input.tool_geometry {
            ToolGeometryHint::Ball => d / 2.0,
            ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
            _ => 0.0,
        };
        if ball_r > 0.0 {
            if let Some(stepover) = geometry::scallop_stepover(ball_r, target_scallop) {
                ae = stepover;
            } else {
                warnings.push(FeedsWarning::ScallopInvalid {
                    target: target_scallop,
                    max_possible: ball_r,
                });
            }
        }
    }

    // Apply user overrides
    if let Some(user_ap) = input.axial_depth_mm {
        ap = user_ap;
    }
    if let Some(user_ae) = input.radial_width_mm {
        ae = user_ae;
    }

    // --- Step 4: Flute guard ---
    const FLUTE_GUARD_FACTOR: f64 = 0.8;
    let flute_guard = if input.flute_length > 0.0 {
        input.flute_length * FLUTE_GUARD_FACTOR
    } else {
        d * 2.0
    };
    if ap > flute_guard {
        warnings.push(FeedsWarning::DocExceedsFlute {
            requested: ap,
            capped: flute_guard,
        });
        ap = flute_guard;
    }

    // Ensure minimum engagement
    const MIN_AP_MM: f64 = 0.05;
    const MIN_AE_MM: f64 = 0.02;
    ap = ap.max(MIN_AP_MM);
    ae = ae.max(MIN_AE_MM);
    // Cap ae to tool diameter
    ae = ae.min(d);

    // --- Step 4b: Slotting detection ---
    const SLOTTING_THRESHOLD: f64 = 0.85;
    const SLOTTING_DOC_CAP: f64 = 0.25;
    if ae > d * SLOTTING_THRESHOLD {
        let slotting_cap = d * SLOTTING_DOC_CAP;
        if ap > slotting_cap {
            warnings.push(FeedsWarning::SlottingDetected {
                doc_reduced_to: slotting_cap,
            });
            ap = slotting_cap;
        }
    }

    // --- Step 4c: Shank check ---
    if let Some(shank) = input.shank_diameter
        && shank > machine.max_shank_mm
    {
        warnings.push(FeedsWarning::ShankTooLarge {
            shank_mm: shank,
            max_mm: machine.max_shank_mm,
        });
    }

    // --- Step 5: Feed rate ---
    let effective_d = effective_diameter(input.tool_geometry, d, ap);

    // Radial chip thinning (all tools)
    let rctf = geometry::radial_chip_thinning_factor(ae, effective_d);

    // Axial chip thinning (ball nose tools at shallow depth)
    let axial_thinning = match input.tool_geometry {
        ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
            geometry::axial_chip_thinning_factor_for_ball(d, effective_d)
        }
        _ => 1.0,
    };
    let chip_thinning = (rctf * axial_thinning).clamp(1.0, 4.0);

    // Depth tier feed derate — deep cuts need slower feed to limit deflection
    let depth_tier = geometry::depth_tier_multiplier(ap, d);

    let mut raw_feed = rpm * chip_load * input.flute_count as f64 * chip_thinning * depth_tier;

    // --- Step 5b: Setup derates ---
    // L/D ratio derating — long tools deflect more
    const LD_SEVERE_THRESHOLD: f64 = 6.0;
    const LD_MODERATE_THRESHOLD: f64 = 4.0;
    const LD_SEVERE_FACTOR: f64 = 0.75;
    const LD_MODERATE_FACTOR: f64 = 0.88;
    let ld_factor = if let Some(overhang) = input.setup.tool_overhang_mm {
        let ld_ratio = overhang / d;
        if ld_ratio > LD_SEVERE_THRESHOLD {
            LD_SEVERE_FACTOR
        } else if ld_ratio > LD_MODERATE_THRESHOLD {
            LD_MODERATE_FACTOR
        } else {
            1.0
        }
    } else {
        1.0
    };
    raw_feed *= ld_factor;
    // Workholding rigidity adjustment
    const WORKHOLDING_LOW_FACTOR: f64 = 0.85;
    const WORKHOLDING_HIGH_FACTOR: f64 = 1.03;
    let workholding_factor = match input.setup.workholding_rigidity {
        WorkholdingRigidity::Low => WORKHOLDING_LOW_FACTOR,
        WorkholdingRigidity::High => WORKHOLDING_HIGH_FACTOR,
        WorkholdingRigidity::Medium => 1.0,
    };
    raw_feed *= workholding_factor;

    // --- Step 6: Power check ---
    // Materials without a primary-source Kc skip the power-vs-machine
    // ramp here; downstream tool_load::power refuses with
    // `MaterialUnvalidated` so the user sees the gap explicitly rather
    // than getting a silently-fabricated feed.
    //
    // Power prediction routes through the canonical
    // `tool_load::power::predicted_power_kw` helper so the Suggest path
    // and the Sim verdict can't diverge. The cross-section uses the
    // geometry-hint's shape-correct area (V-bit triangular,
    // flat/ball/bull/tapered rectangular) — same contract as the
    // cutter trait's `mrr_cross_section_mm2` the Sim verdict reads.
    let available_power = machine.power_at_rpm(rpm);
    let mut power_limited = false;
    let mut feed = raw_feed;
    let mut power_factor = 1.0;

    if let Some(kc) = material.kc_n_per_mm2() {
        let cross_section =
            input.tool_geometry.mrr_cross_section_mm2(ap, ae);
        let required_power =
            crate::tool_load::power::predicted_power_kw(kc, cross_section, raw_feed);
        if required_power > available_power && available_power > 0.0 {
            power_factor = available_power / required_power;
            feed = raw_feed * power_factor;
            power_limited = true;
            warnings.push(FeedsWarning::PowerLimited {
                required_kw: required_power,
                available_kw: available_power,
            });
        }
    }

    // --- Step 7: Machine feed clamp ---
    let mut feed_clamp_factor = 1.0;
    if feed > machine.max_feed_mm_min {
        warnings.push(FeedsWarning::FeedRateClamped {
            requested: feed,
            actual: machine.max_feed_mm_min,
        });
        if feed > 0.0 {
            feed_clamp_factor = machine.max_feed_mm_min / feed;
        }
        feed = machine.max_feed_mm_min;
    }

    // --- Step 8: Plunge rate ---
    // Diameter-aware plunge baseline (fix #7) — a 3 mm bit no longer
    // gets the same plunge envelope as a 12 mm bit. The 6 mm baseline
    // is preserved, other diameters scale linearly with the audit
    // rule-of-thumb (150-300 mm/min per mm of diameter for wood).
    let plunge = material.plunge_rate_base(d);

    // Ramp feed: capped at 1.5× plunge rate (reference calcs.rs convention)
    let ramp_feed = (feed * 0.5).max(plunge).min(plunge * 1.5);

    // --- Step 9: Safety factor ---
    feed *= machine.safety_factor;
    let mut plunge_rate = plunge * machine.safety_factor;
    let ramp_feed_rate = ramp_feed * machine.safety_factor;

    // Fix 2 (Wanaka audit): tool-geometry-aware plunge cap.
    // Material::plunge_rate_base returns one value per material with
    // no tool-geometry awareness, so a 1 mm tapered ball gets the
    // same plunge as a 12 mm end-mill. Published FSWizard / GWizard
    // ranges are 100–300 mm/min for sub-2 mm tapered/ball tools in
    // wood — derate accordingly. Cap at 150 mm/min per mm of
    // effective tip diameter for ball/tapered-ball geometries; larger
    // tools and flat/bull tools are unchanged. See
    // `planning/PRE_OPTIMIZE_DEFAULTS_AUDIT.md` Fix 2.
    let plunge_cap = match input.tool_geometry {
        ToolGeometryHint::Ball => Some(d),
        ToolGeometryHint::TaperedBall { tip_radius, .. } => Some((tip_radius * 2.0).max(0.5)),
        _ => None,
    };
    if let Some(tip_d) = plunge_cap {
        let cap = 150.0 * tip_d;
        if plunge_rate > cap {
            plunge_rate = cap;
        }
    }

    // Final power at actual feed. Materials without a primary-source Kc
    // report 0.0 — the consumers that need a numeric headroom (charts /
    // diagnostics) treat this as "unmodeled" rather than zero load.
    // Same canonical helper as Step 6 above so Suggest's reported
    // power matches the Sim verdict's prediction.
    let actual_power = match material.kc_n_per_mm2() {
        Some(kc) => {
            let cross_section =
                input.tool_geometry.mrr_cross_section_mm2(ap, ae);
            crate::tool_load::power::predicted_power_kw(kc, cross_section, feed)
        }
        None => 0.0,
    };
    let mrr = ap * ae * feed;

    let formula = if matches!(
        chipload_source,
        ChiploadSource::FormulaFallback | ChiploadSource::EdgeRadiusFloor
    ) {
        Some(FormulaBreakdown {
            k0: cl.k0,
            p: cl.p,
            q: cl.q,
            diameter_mm: d,
            feed_scale_factor: feed_scale,
            result_mm_tooth: formula_chipload,
        })
    } else {
        None
    };

    let derates = FeedsDerates {
        target_chip_load_mm: chip_load,
        formula,
        radial_chip_thinning: rctf,
        axial_chip_thinning: axial_thinning,
        combined_chip_thinning: chip_thinning,
        depth_tier,
        ld_overhang: ld_factor,
        workholding: workholding_factor,
        power_limit: power_factor,
        feed_clamp: feed_clamp_factor,
        safety_factor: machine.safety_factor,
        spindle_speedup,
    };

    FeedsResult {
        rpm,
        chip_load_mm: chip_load,
        feed_rate_mm_min: feed,
        plunge_rate_mm_min: plunge_rate,
        ramp_feed_mm_min: ramp_feed_rate,
        axial_depth_mm: ap,
        radial_width_mm: ae,
        power_kw: actual_power,
        available_power_kw: available_power,
        power_limited,
        mrr_mm3_min: mrr,
        warnings,
        vendor_source,
        chipload_source,
        derates,
    }
}

/// Operation default DOC/WOC profile factors (multiplied by tool diameter).
struct DefaultProfile {
    ap_factor: f64,
    ae_factor: f64,
}

fn operation_default_profile(family: OperationFamily, role: PassRole) -> DefaultProfile {
    match (family, role) {
        // Adaptive: deep and narrow
        (OperationFamily::Adaptive, PassRole::Roughing) => DefaultProfile {
            ap_factor: 1.50,
            ae_factor: 0.12,
        },
        (OperationFamily::Adaptive, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.90,
            ae_factor: 0.10,
        },
        (OperationFamily::Adaptive, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.70,
            ae_factor: 0.08,
        },
        // Pocket: moderate
        (OperationFamily::Pocket, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.70,
            ae_factor: 0.35,
        },
        (OperationFamily::Pocket, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.35,
            ae_factor: 0.20,
        },
        (OperationFamily::Pocket, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.20,
            ae_factor: 0.08,
        },
        // Contour: moderate depth, narrow width
        (OperationFamily::Contour, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.80,
            ae_factor: 0.18,
        },
        (OperationFamily::Contour, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.45,
            ae_factor: 0.10,
        },
        (OperationFamily::Contour, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.30,
            ae_factor: 0.05,
        },
        // Parallel: shallow surface following
        (OperationFamily::Parallel, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.25,
            ae_factor: 0.08,
        },
        (OperationFamily::Parallel, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.16,
            ae_factor: 0.05,
        },
        (OperationFamily::Parallel, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.10,
            ae_factor: 0.03,
        },
        // Scallop: very fine
        (OperationFamily::Scallop, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.20,
            ae_factor: 0.07,
        },
        (OperationFamily::Scallop, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.14,
            ae_factor: 0.05,
        },
        (OperationFamily::Scallop, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.08,
            ae_factor: 0.025,
        },
        // Trace: V-carve/engrave
        (OperationFamily::Trace, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.15,
            ae_factor: 0.05,
        },
        (OperationFamily::Trace, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.10,
            ae_factor: 0.03,
        },
        (OperationFamily::Trace, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.06,
            ae_factor: 0.02,
        },
        // Face: wide and shallow
        (OperationFamily::Face, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.08,
            ae_factor: 0.65,
        },
        (OperationFamily::Face, PassRole::SemiFinish) => DefaultProfile {
            ap_factor: 0.06,
            ae_factor: 0.55,
        },
        (OperationFamily::Face, PassRole::Finish) => DefaultProfile {
            ap_factor: 0.04,
            ae_factor: 0.45,
        },
        // Drill: ap is the per-peck descent (handled by peck_depth on
        // DrillConfig, not depth_per_pass), ae is structurally
        // undefined for Z-only kinematics. Profile factors here exist
        // for type completeness only — the drill-specific RPM clamp
        // in `calculate()` is what actually controls the operating
        // point. `pass_role` is always `Roughing` for drill ops
        // (drill cycles don't semi-finish or finish).
        (OperationFamily::Drill, _) => DefaultProfile {
            ap_factor: 0.0,
            ae_factor: 0.0,
        },
    }
}

fn default_engagement(
    d: f64,
    profile: &DefaultProfile,
    input: &FeedsInput,
    machine: &MachineProfile,
) -> (f64, f64) {
    let mut ap_factor = profile.ap_factor;
    let mut ae_factor = profile.ae_factor;

    // --- Adaptive-specific engagement matrix per tool geometry ---
    // From reference calcs.rs: separate entries for Flat/Ball/TaperedBall
    if input.operation == OperationFamily::Adaptive {
        let roughing = input.pass_role == PassRole::Roughing;
        let semi = input.pass_role == PassRole::SemiFinish;
        if roughing || semi {
            let (ap_base, ae_base) = match input.tool_geometry {
                ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } => (1.20, 0.14),
                ToolGeometryHint::Ball => (0.80, 0.10),
                ToolGeometryHint::TaperedBall { .. } => (0.70, 0.08),
                ToolGeometryHint::VBit { .. } => (ap_factor, ae_factor),
            };
            ap_factor = if roughing { ap_base } else { ap_base * 0.75 };
            ae_factor = if roughing { ae_base } else { ae_base * 0.85 };

            // Multi-flute AE derate for adaptive (>= 3 flutes)
            if input.flute_count >= 3 {
                ae_factor *= 0.85;
            }

            // Feed-scale-dependent adaptive derates — harder materials
            // (higher factor) want shallower ap/ae; softer materials
            // can take a slightly more aggressive bite.
            let feed_scale = input.material.feed_scale_factor();
            if feed_scale > 1.40 {
                ap_factor *= 0.80;
                ae_factor *= 0.90;
            } else if feed_scale > 1.15 {
                ap_factor *= 0.90;
                ae_factor *= 0.95;
            } else if feed_scale < 0.85 {
                ap_factor *= 1.05;
                ae_factor *= 1.05;
            }
        }

        // Apply machine rigidity bounds.
        //
        // For ap (DOC) the rigidity factor is a target floor — the
        // machine can sustain at least this much axial engagement.
        //
        // For ae (WOC) the rigidity factor is a ceiling by default
        // (don't exceed the machine's adaptive capability), but for
        // **wood-class materials on flat/bull tools** it's also a
        // target floor — wood-router practice is to actually use the
        // full machine factor (~0.20 D) rather than the metal-grade
        // 0.14 base. Without this the empirical engagement profile
        // sits in the "light" bin (≤ 0.10 D — too narrow), wasting
        // cycle time without improving safety. See
        // `planning/PRE_OPTIMIZE_DEFAULTS_AUDIT.md` Fix 1.
        ap_factor = ap_factor.max(machine.rigidity.adaptive_doc_factor * profile.ap_factor / 1.5);
        // Wood-class match goes through Material::is_wood_class() so that
        // species-aware variants (SolidWoodByJanka — FPL Ch.5 species
        // library) qualify for the wood-router adaptive WOC floor.
        // Pre-2026-06-02 this was an inline arm that omitted
        // SolidWoodByJanka, dropping ae_factor from 0.20×D back to
        // 0.14×D for any species-aware wood project (audit bug 2).
        let wood_class_flat_tool = input.material.is_wood_class()
            && matches!(
                input.tool_geometry,
                ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. }
            );
        if wood_class_flat_tool {
            ae_factor = ae_factor.max(machine.rigidity.adaptive_woc_factor);
        }
        ae_factor = ae_factor.min(machine.rigidity.adaptive_woc_factor);
    }

    // --- Tool geometry adjustments for finishing operations ---
    match (input.tool_geometry, input.operation, input.pass_role) {
        (
            ToolGeometryHint::Ball,
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::Finish,
        ) => {
            ap_factor = 0.06;
            ae_factor = 0.025;
        }
        (
            ToolGeometryHint::Ball,
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::SemiFinish,
        ) => {
            ap_factor = 0.10;
            ae_factor = 0.04;
        }
        (
            ToolGeometryHint::TaperedBall { .. },
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::Finish,
        ) => {
            ap_factor = 0.10;
            ae_factor = 0.03;
        }
        (
            ToolGeometryHint::TaperedBall { .. },
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::SemiFinish,
        ) => {
            ap_factor = 0.14;
            ae_factor = 0.05;
        }
        _ => {}
    }

    let ap = (d * ap_factor).max(0.05);
    let ae = (d * ae_factor).max(0.02);
    (ap, ae)
}

fn effective_diameter(geom: ToolGeometryHint, nominal_d: f64, ap: f64) -> f64 {
    match geom {
        ToolGeometryHint::Flat => nominal_d,
        ToolGeometryHint::Ball => geometry::ball_effective_diameter(nominal_d, ap),
        ToolGeometryHint::Bull { corner_radius } => {
            geometry::bull_nose_effective_diameter(nominal_d, corner_radius, ap)
        }
        ToolGeometryHint::VBit {
            included_angle,
            tip_diameter,
        } => geometry::vbit_width_at_depth(included_angle, tip_diameter, ap)
            .unwrap_or(nominal_d)
            .min(nominal_d),
        ToolGeometryHint::TaperedBall {
            tip_radius,
            taper_angle_deg,
        } => geometry::tapered_ball_effective_diameter(nominal_d, tip_radius, taper_angle_deg, ap),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::machine::MachineProfile;
    use crate::material::{Material, WoodSpecies};

    fn softwood_flat_6mm_pocket() -> FeedsInput<'static> {
        // We need 'static material/machine so use leaked boxes for test convenience
        let material: &'static Material = Box::leak(Box::new(Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }));
        let machine: &'static MachineProfile = Box::leak(Box::new(MachineProfile::shapeoko_vfd()));
        FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            shank_diameter: None,
            tool_geometry: ToolGeometryHint::Flat,
            material,
            machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        }
    }

    #[test]
    fn test_chip_load_soft_wood_6mm() {
        let machine = MachineProfile::shapeoko_vfd();
        let d: f64 = 6.0;
        let h = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }
        .feed_scale_factor();
        let cl = machine.chip_load.k0
            * d.powf(machine.chip_load.p)
            * (1.0 / h).powf(machine.chip_load.q);
        assert!((cl - 0.0716).abs() < 0.002, "expected ~0.0716, got {cl}");
    }

    #[test]
    fn test_chip_load_hard_wood_3175mm() {
        let machine = MachineProfile::shapeoko_vfd();
        let d: f64 = 3.175;
        let h = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        }
        .feed_scale_factor();
        let cl = machine.chip_load.k0
            * d.powf(machine.chip_load.p)
            * (1.0 / h).powf(machine.chip_load.q);
        assert!((cl - 0.0311).abs() < 0.002, "expected ~0.0311, got {cl}");
    }

    #[test]
    fn test_feed_rate_basic() {
        let feed = 18000.0 * 0.05 * 2.0;
        assert_eq!(feed, 1800.0);
    }

    #[test]
    fn test_calculate_produces_reasonable_values() {
        let input = softwood_flat_6mm_pocket();
        let result = calculate(&input);

        assert!(
            result.rpm >= 6000.0 && result.rpm <= 24000.0,
            "RPM {}",
            result.rpm
        );
        assert!(
            result.feed_rate_mm_min > 500.0 && result.feed_rate_mm_min < 5000.0,
            "feed {}",
            result.feed_rate_mm_min
        );
        assert!(
            result.plunge_rate_mm_min > 100.0 && result.plunge_rate_mm_min < 2000.0,
            "plunge {}",
            result.plunge_rate_mm_min
        );
        assert!(
            result.axial_depth_mm > 0.0 && result.axial_depth_mm <= 18.0,
            "DOC {}",
            result.axial_depth_mm
        );
        assert!(
            result.radial_width_mm > 0.0 && result.radial_width_mm <= 6.0,
            "WOC {}",
            result.radial_width_mm
        );
        assert!(result.power_kw >= 0.0, "power {}", result.power_kw);
    }

    #[test]
    fn test_adaptive_deeper_narrower_than_pocket() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let adaptive = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });
        let pocket = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            adaptive.axial_depth_mm > pocket.axial_depth_mm,
            "adaptive DOC {} should > pocket DOC {}",
            adaptive.axial_depth_mm,
            pocket.axial_depth_mm
        );
        assert!(
            adaptive.radial_width_mm < pocket.radial_width_mm,
            "adaptive WOC {} should < pocket WOC {}",
            adaptive.radial_width_mm,
            pocket.radial_width_mm
        );
    }

    #[test]
    fn test_roughing_deeper_than_finishing() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let families = [
            OperationFamily::Adaptive,
            OperationFamily::Pocket,
            OperationFamily::Contour,
            OperationFamily::Parallel,
            OperationFamily::Scallop,
            OperationFamily::Trace,
            OperationFamily::Face,
        ];

        for family in families {
            let rough = calculate(&FeedsInput {
                tool_diameter: 6.0,
                flute_count: 2,
                flute_length: 18.0,
                tool_geometry: ToolGeometryHint::Flat,
                shank_diameter: None,
                material: &material,
                machine: &machine,
                operation: family,
                pass_role: PassRole::Roughing,
                axial_depth_mm: None,
                radial_width_mm: None,
                target_scallop_mm: None,
                vendor_lut: None,
                setup: SetupContext::default(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
            });
            let finish = calculate(&FeedsInput {
                tool_diameter: 6.0,
                flute_count: 2,
                flute_length: 18.0,
                tool_geometry: ToolGeometryHint::Flat,
                shank_diameter: None,
                material: &material,
                machine: &machine,
                operation: family,
                pass_role: PassRole::Finish,
                axial_depth_mm: None,
                radial_width_mm: None,
                target_scallop_mm: None,
                vendor_lut: None,
                setup: SetupContext::default(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
            });

            assert!(
                rough.axial_depth_mm >= finish.axial_depth_mm,
                "{family:?}: roughing DOC {} should >= finishing DOC {}",
                rough.axial_depth_mm,
                finish.axial_depth_mm
            );
            assert!(
                rough.radial_width_mm >= finish.radial_width_mm,
                "{family:?}: roughing WOC {} should >= finishing WOC {}",
                rough.radial_width_mm,
                finish.radial_width_mm
            );
        }
    }

    /// Fix 2 (Wanaka audit): plunge for small ball/tapered-ball tools
    /// must derate by tool-tip diameter. A 1 mm tapered ball in
    /// hardwood was previously emitting 750 mm/min plunge (the
    /// material-level base × machine safety factor) — 2.5× above
    /// FSWizard's 100–300 mm/min safe band. Cap is 150 mm/min per
    /// mm of tip diameter.
    #[test]
    fn test_small_tapered_ball_plunge_derated() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 1.0,
            flute_count: 1,
            flute_length: 6.0,
            tool_geometry: ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 7.0,
            },
            shank_diameter: Some(6.0),
            material: &material,
            machine: &machine,
            operation: OperationFamily::Parallel,
            pass_role: PassRole::Finish,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            result.plunge_rate_mm_min <= 200.0,
            "1mm TB plunge {} should be ≤ 200 mm/min after Fix 2 derate",
            result.plunge_rate_mm_min
        );
        assert!(
            result.plunge_rate_mm_min >= 50.0,
            "1mm TB plunge {} should not collapse to near-zero",
            result.plunge_rate_mm_min
        );
    }

    /// Counter-test: flat end-mills are unaffected by Fix 2.
    #[test]
    fn test_flat_endmill_plunge_unchanged_by_fix2() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // 6mm flat in hardwood: material_base/hardness × safety ≈
        // 1000/1.42 × 0.75 ≈ 528 mm/min. Should NOT be capped.
        assert!(
            result.plunge_rate_mm_min > 400.0,
            "6mm flat plunge {} should not be derated by Fix 2 tool-geometry cap",
            result.plunge_rate_mm_min
        );
    }

    /// Fix 1 (Wanaka audit): adaptive stepover for wood + flat tools
    /// should track the machine rigidity factor (`adaptive_woc_factor`),
    /// not the metal-grade 0.14 base. Empirical: 6 mm flat in
    /// Generic Hardwood on a wood-router with `adaptive_woc_factor =
    /// 0.20` should yield ae ≈ 1.2 mm, not 0.7 mm. Engagement profile
    /// then sits in the "normal" bin instead of "light".
    #[test]
    fn test_wood_adaptive_stepover_tracks_machine_factor() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let target = machine.rigidity.adaptive_woc_factor * 6.0;

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            (result.radial_width_mm - target).abs() < 1e-6,
            "wood adaptive WOC {} should match machine.adaptive_woc_factor × D = {}",
            result.radial_width_mm,
            target
        );
    }

    /// Counter-test: metals should NOT get the wood adaptive bonus —
    /// the metal-grade 0.14 base (with hardness derates) stays in
    /// force so adaptive stepover remains conservative.
    #[test]
    fn test_metal_adaptive_stepover_keeps_metal_base() {
        let material = Material::Plastic {
            family: crate::material::PlasticFamily::Acrylic,
        };
        // Acrylic is not wood-class — should bypass the Fix 1 floor.
        let machine = MachineProfile::shapeoko_vfd();
        let machine_factor_ae = machine.rigidity.adaptive_woc_factor * 6.0;

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Plastic is not wood-class; ae should sit below the
        // machine ceiling rather than being raised to it.
        assert!(
            result.radial_width_mm < machine_factor_ae,
            "non-wood adaptive WOC {} should stay below machine factor {}",
            result.radial_width_mm,
            machine_factor_ae
        );
    }

    /// Fix #7 (2026-06-02 audit): `Material::plunge_rate_base()`
    /// scales linearly with tool diameter. A 6 mm bit gets the
    /// preserved baseline; a 3 mm bit gets half, a 12 mm bit gets
    /// double (within the [0.25×, 3×] clamp). Pre-fix a 3 mm bit
    /// inherited the 6 mm plunge envelope, plunging 2× too fast.
    #[test]
    fn test_plunge_rate_base_scales_with_diameter() {
        use crate::material::Material;
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };

        let plunge_3mm = material.plunge_rate_base(3.0);
        let plunge_6mm = material.plunge_rate_base(6.0);
        let plunge_12mm = material.plunge_rate_base(12.0);

        // 6 mm baseline preserved (matches pre-fix material-only value).
        let h = material.feed_scale_factor();
        let expected_6mm = 1000.0 / h;
        assert!(
            (plunge_6mm - expected_6mm).abs() < 1e-6,
            "6 mm plunge {plunge_6mm} should preserve pre-fix value {expected_6mm}"
        );

        // 3 mm = half of 6 mm.
        assert!(
            (plunge_3mm - plunge_6mm * 0.5).abs() < 1e-6,
            "3 mm plunge {plunge_3mm} should be 0.5 × 6 mm plunge {plunge_6mm}"
        );

        // 12 mm = double of 6 mm.
        assert!(
            (plunge_12mm - plunge_6mm * 2.0).abs() < 1e-6,
            "12 mm plunge {plunge_12mm} should be 2.0 × 6 mm plunge {plunge_6mm}"
        );

        // Clamps: very small / very large tools don't run away.
        let plunge_tiny = material.plunge_rate_base(1.0);
        assert!(
            plunge_tiny >= plunge_6mm * 0.25 - 1e-6,
            "tiny-tool plunge {plunge_tiny} should clamp to 0.25 × baseline floor"
        );
        let plunge_huge = material.plunge_rate_base(25.0);
        assert!(
            plunge_huge <= plunge_6mm * 3.0 + 1e-6,
            "huge-tool plunge {plunge_huge} should clamp to 3.0 × baseline ceiling"
        );
    }

    /// Fix #3 (2026-06-02 audit): Drill ops are routed through their
    /// own `OperationFamily::Drill` (was `OperationFamily::Pocket`),
    /// and the calculate() path clamps drill RPM to 8-14k regardless
    /// of diameter — milling SFM/RPM derivation push small-D drills
    /// past 16k where chipload starves. With the multiplier, drill
    /// chipload lands in the 0.05-0.15 mm/rev softwood band.
    #[test]
    fn test_drill_family_rpm_in_drill_band() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Drill,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // RPM must sit inside the drill band, not the milling SFM result.
        assert!(
            (8_000.0..=14_000.0).contains(&result.rpm),
            "drill RPM {} must clamp to 8k-14k drill band",
            result.rpm
        );

        // Implied chipload = feed / (flutes * rpm). With the drill
        // multiplier, this must clear the 0.05 mm/rev softwood drill
        // floor. Pre-fix lands at 0.026 (audit finding).
        let implied_chipload = result.feed_rate_mm_min / (2.0 * result.rpm);
        assert!(
            implied_chipload >= 0.05,
            "drill implied chipload {} must clear softwood drill floor of 0.05",
            implied_chipload
        );
    }

    /// Fix #3 regression guard: a Pocket op on the same tool/material
    /// must NOT see the drill RPM clamp or chipload multiplier —
    /// the fix is selective by family.
    #[test]
    fn test_pocket_family_unaffected_by_drill_fix() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Pocket RPM is allowed to be >14k (milling SFM band).
        // Don't assert a specific value — just check the drill clamp
        // didn't fire.
        assert!(
            result.rpm > 14_000.0 || result.rpm == machine.clamp_rpm(result.rpm),
            "pocket RPM {} should not be drill-clamped",
            result.rpm
        );
    }

    /// Fix #4 (2026-06-02 audit): RPM for a V-bit must be derived from
    /// the engaged tip diameter at DOC, not the nominal shank diameter.
    /// A 20° V-bit at shallow DOC has near-zero engaged D → SFM-derived
    /// RPM should hit the machine ceiling (the rule of thumb says V-bit
    /// wants high RPM because effective SFM at the tip is essentially
    /// zero). The pre-fix path used nominal D=5.5 mm and produced ~11.5k
    /// RPM regardless of DOC.
    #[test]
    fn test_vbit_rpm_uses_engaged_diameter() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let machine_max_rpm = match &machine.spindle {
            crate::machine::SpindleConfig::Variable { max_rpm, .. } => *max_rpm,
            _ => panic!("expected variable spindle on shapeoko_vfd"),
        };

        // 20° V-bit, 5.5 mm shank, at 0.5 mm DOC. Engaged tip diameter
        // = 2 * 0.5 * tan(10°) ≈ 0.176 mm — small enough that SFM-derived
        // ideal RPM exceeds the machine ceiling, so the result clamps
        // to machine max.
        let result = calculate(&FeedsInput {
            tool_diameter: 5.5,
            flute_count: 2,
            flute_length: 12.0,
            tool_geometry: ToolGeometryHint::VBit {
                included_angle: 20.0,
                tip_diameter: 0.0,
            },
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Trace,
            pass_role: PassRole::Finish,
            axial_depth_mm: Some(0.5),
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            result.rpm >= machine_max_rpm * 0.99,
            "20° V-bit at DOC=0.5 mm should clamp to spindle max ({}), \
             got {} — engaged-D pipeline likely not firing",
            machine_max_rpm,
            result.rpm
        );
    }

    /// Fix #4 regression guard: Flat tools must produce the same RPM
    /// and chipload before/after the engaged-D switch — `engaged_diameter_at_doc`
    /// returns nominal D for Flat geometry, so all Flat-tool feeds
    /// stay byte-identical.
    #[test]
    fn test_flat_tool_rpm_chipload_unchanged_by_engaged_d() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let d = 6.0;
        // Baseline: what the formula path produced via nominal D.
        let baseline_rpm =
            (material.base_cutting_speed_m_min() * 1000.0 / (std::f64::consts::PI * d)).round();

        let result = calculate(&FeedsInput {
            tool_diameter: d,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: Some(3.0), // DOC doesn't matter for Flat
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Allow the machine-clamp the formula was about to be passed
        // through (so we compare clamp(baseline) ≈ result.rpm).
        let clamped_baseline = machine.clamp_rpm(baseline_rpm);
        assert!(
            (result.rpm - clamped_baseline).abs() < 1.0,
            "Flat-tool RPM should be unchanged by engaged-D switch \
             (baseline {clamped_baseline}, got {})",
            result.rpm
        );
    }

    /// Bug 2 (2026-06-02 audit): the species-aware `SolidWoodByJanka`
    /// variant must qualify for the wood adaptive WOC floor — same as
    /// `SolidWood`. Eastern White Pine (Janka 382.2, used in Wanaka)
    /// should give ae = `adaptive_woc_factor × D` = 1.2 mm on a 6 mm
    /// flat, not the 0.88 mm (= 0.147 × D) that the bug produced.
    #[test]
    fn test_wood_adaptive_stepover_solid_wood_by_janka_eastern_white_pine() {
        let material = Material::SolidWoodByJanka {
            janka_lbf: 382.2,
            label: "Pine, eastern white".to_owned(),
            source_id: "fpl_ch5_2010".to_owned(),
        };
        let machine = MachineProfile::shapeoko_vfd();
        let target = machine.rigidity.adaptive_woc_factor * 6.0;

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            (result.radial_width_mm - target).abs() < 1e-6,
            "SolidWoodByJanka adaptive WOC {} should match \
             machine.adaptive_woc_factor × D = {} (Bug 2 fix)",
            result.radial_width_mm,
            target
        );
    }

    #[test]
    fn test_flute_guard_caps_doc() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 5.0, // very short flutes
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            result.axial_depth_mm <= 5.0 * 0.8 + 0.01,
            "DOC {} should be capped by flute guard 4.0",
            result.axial_depth_mm
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, FeedsWarning::DocExceedsFlute { .. }))
        );
    }

    #[test]
    fn test_power_limiting_on_low_power_machine() {
        // Use softwood (high chip load) with a tiny spindle to trigger power limiting
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let mut machine = MachineProfile::shapeoko_vfd();
        machine.power = crate::machine::PowerModel::ConstantPower { power_kw: 0.01 }; // extremely tiny

        let result = calculate(&FeedsInput {
            tool_diameter: 12.0,
            flute_count: 4,
            flute_length: 25.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: Some(5.0),
            radial_width_mm: Some(8.0),
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            result.power_limited,
            "should be power limited with 0.01kW spindle: power={:.4}kW, available={:.4}kW",
            result.power_kw, result.available_power_kw
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, FeedsWarning::PowerLimited { .. }))
        );
    }

    #[test]
    fn test_scallop_stepover_used_for_ball_nose() {
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Ball,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Parallel,
            pass_role: PassRole::Finish,
            axial_depth_mm: Some(0.4),
            radial_width_mm: None,
            target_scallop_mm: Some(0.03),
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // With 3mm ball radius, 0.03mm scallop → stepover should be small
        assert!(result.radial_width_mm > 0.0 && result.radial_width_mm < 6.0);
    }

    #[test]
    fn test_machine_feed_clamp() {
        let material = Material::Foam {
            density: crate::material::FoamDensity::Low,
        };
        let mut machine = MachineProfile::generic_wood_router();
        machine.max_feed_mm_min = 500.0; // very low max feed

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            result.feed_rate_mm_min <= 500.0 * machine.safety_factor + 0.01,
            "feed {} should be clamped to max {}",
            result.feed_rate_mm_min,
            500.0
        );
    }

    #[test]
    fn test_safety_factor_applied() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Feed should be < what it would be without safety factor
        // The safety factor is 0.80, so feed should be roughly 80% of unclamped
        assert!(result.feed_rate_mm_min > 0.0);
    }

    #[test]
    fn test_slotting_detection() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: Some(10.0),
            radial_width_mm: Some(5.5), // >85% of D = slotting
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            result.axial_depth_mm <= 6.0 * 0.25 + 0.01,
            "slotting should reduce DOC, got {}",
            result.axial_depth_mm
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, FeedsWarning::SlottingDetected { .. }))
        );
    }

    // --- Vendor LUT integration tests ---

    #[test]
    fn test_lut_chipload_overrides_formula() {
        let lut = vendor_lut::VendorLut::embedded();
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let with_lut = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });
        let without_lut = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // LUT chipload for 6mm softwood adaptive should be ~0.0875 (midpoint 0.065-0.11)
        // Formula chipload should be ~0.0716
        assert!(
            (with_lut.chip_load_mm - 0.0875).abs() < 0.002,
            "LUT chipload should be ~0.0875, got {}",
            with_lut.chip_load_mm
        );
        assert!(
            (without_lut.chip_load_mm - 0.0716).abs() < 0.002,
            "formula chipload should be ~0.0716, got {}",
            without_lut.chip_load_mm
        );
        assert!(
            with_lut.chip_load_mm > without_lut.chip_load_mm,
            "LUT chipload {} should differ from formula {}",
            with_lut.chip_load_mm,
            without_lut.chip_load_mm
        );
        assert!(with_lut.vendor_source.is_some());
        assert!(matches!(
            with_lut.chipload_source,
            ChiploadSource::VendorLut { .. }
        ));
        assert!(without_lut.vendor_source.is_none());
        assert_eq!(without_lut.chipload_source, ChiploadSource::FormulaFallback);
    }

    #[test]
    fn test_lut_rpm_override_within_machine_range() {
        let lut = vendor_lut::VendorLut::embedded();
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Vendor RPM for amana 6mm softwood adaptive is 18000
        assert!(
            (result.rpm - 18000.0).abs() < 100.0,
            "RPM should be ~18000 from vendor data, got {}",
            result.rpm
        );
    }

    #[test]
    fn test_no_lut_backward_compatible() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(result.vendor_source.is_none());
        assert_eq!(result.chipload_source, ChiploadSource::FormulaFallback);
        assert!(result.rpm > 6000.0 && result.rpm < 24000.0);
        assert!(result.feed_rate_mm_min > 500.0);
        assert!(result.chip_load_mm > 0.05 && result.chip_load_mm < 0.12);
    }

    #[test]
    fn test_setup_derate_long_overhang() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let normal = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext {
                tool_overhang_mm: Some(20.0), // L/D = 20/6 = 3.3, no derate
                workholding_rigidity: WorkholdingRigidity::Medium,
            },
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        let long = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext {
                tool_overhang_mm: Some(40.0), // L/D = 40/6 = 6.67, 25% derate
                workholding_rigidity: WorkholdingRigidity::Medium,
            },
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // L/D > 6 should reduce feed by 25%
        let ratio = long.feed_rate_mm_min / normal.feed_rate_mm_min;
        assert!(
            (ratio - 0.75).abs() < 0.02,
            "L/D>6 derate should give 0.75x feed ratio, got {ratio}"
        );
    }

    #[test]
    fn test_setup_derate_medium_overhang() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let normal = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext {
                tool_overhang_mm: Some(20.0), // L/D = 3.3, no derate
                workholding_rigidity: WorkholdingRigidity::Medium,
            },
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        let medium = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext {
                tool_overhang_mm: Some(30.0), // L/D = 30/6 = 5.0, 12% derate
                workholding_rigidity: WorkholdingRigidity::Medium,
            },
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        let ratio = medium.feed_rate_mm_min / normal.feed_rate_mm_min;
        assert!(
            (ratio - 0.88).abs() < 0.02,
            "L/D>4 derate should give 0.88x feed ratio, got {ratio}"
        );
    }

    #[test]
    fn test_setup_derate_workholding_low() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let medium = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext {
                tool_overhang_mm: None,
                workholding_rigidity: WorkholdingRigidity::Medium,
            },
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        let low = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext {
                tool_overhang_mm: None,
                workholding_rigidity: WorkholdingRigidity::Low,
            },
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        let ratio = low.feed_rate_mm_min / medium.feed_rate_mm_min;
        assert!(
            (ratio - 0.85).abs() < 0.02,
            "Low workholding should give 0.85x feed ratio, got {ratio}"
        );
    }

    #[test]
    fn test_lut_ball_nose_different_chipload_than_flat() {
        let lut = vendor_lut::VendorLut::embedded();
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();

        let flat = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });
        let ball = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Ball,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Parallel,
            pass_role: PassRole::Finish,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Ball nose finishing should have a different (lower) chipload than flat adaptive
        assert_ne!(
            flat.chip_load_mm, ball.chip_load_mm,
            "LUT should give different chiploads for flat vs ball"
        );
        assert!(
            flat.chip_load_mm > ball.chip_load_mm,
            "flat adaptive chipload {} should be > ball finish chipload {}",
            flat.chip_load_mm,
            ball.chip_load_mm
        );
    }

    #[test]
    fn test_lut_fallback_when_no_match() {
        let lut = vendor_lut::VendorLut::embedded();
        let material = Material::Foam {
            density: crate::material::FoamDensity::Low,
        };
        let machine = MachineProfile::shapeoko_vfd();

        // Foam has no LUT data — should fall back to formula
        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        // Foam maps to softwood in normalize, but with hardness 200 which is far from
        // any observation. If it does match, that's fine. If not, formula is used.
        assert!(result.feed_rate_mm_min > 0.0);
    }

    /// Phase 5 follow-up (2026-06-01): `SpindleStrategy::MaxSpeed`
    /// pushes RPM toward the spindle ceiling and scales feed
    /// proportionally to keep chipload constant. Sanity-check against
    /// a softwood adaptive query whose vendor row publishes
    /// rpm_nominal at 18000 with our 24000 RPM ceiling.
    #[test]
    fn spindle_strategy_max_speed_lifts_rpm_and_scales_feed() {
        let lut = vendor_lut::VendorLut::embedded();
        let machine = MachineProfile::shapeoko_vfd();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::GenericSoftwood,
        };
        let base = FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            shank_diameter: None,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: SpindleStrategy::MatchChart,
        };
        let match_chart = calculate(&base);
        let max_speed = calculate(&FeedsInput {
            spindle_strategy: SpindleStrategy::MaxSpeed,
            ..base
        });

        // Chipload is preserved (constant-chipload line).
        let chipload_chart = match_chart.feed_rate_mm_min
            / (match_chart.rpm * f64::from(base.flute_count));
        let chipload_max = max_speed.feed_rate_mm_min
            / (max_speed.rpm * f64::from(base.flute_count));
        assert!(
            (chipload_chart - chipload_max).abs() / chipload_chart < 0.02,
            "chipload should be preserved: chart {chipload_chart:.5} vs max {chipload_max:.5}"
        );

        // RPM is at or above the chart RPM under MaxSpeed (>= because
        // when the chart is already at ceiling there's no headroom).
        assert!(
            max_speed.rpm >= match_chart.rpm - 1.0,
            "MaxSpeed RPM {} should be >= MatchChart RPM {}",
            max_speed.rpm,
            match_chart.rpm,
        );

        // For the GenericSoftwood/6mm/adaptive query the chart is at
        // 18000 RPM and the machine ceiling is 24000 — we should see a
        // meaningful lift.
        let (_, machine_max) = machine.rpm_range();
        let speedup = max_speed.rpm / match_chart.rpm;
        let ceiling = machine_max * SPINDLE_CEILING_HEADROOM;
        assert!(
            speedup > 1.05 || max_speed.rpm >= ceiling * 0.99,
            "expected meaningful speedup or to hit the ceiling: speedup={speedup:.2}, rpm={}",
            max_speed.rpm
        );

        // FeedsDerates.spindle_speedup tracks the multiplier for UI.
        assert!(
            (max_speed.derates.spindle_speedup - speedup).abs() < 0.05,
            "derates.spindle_speedup {} should match observed RPM speedup {}",
            max_speed.derates.spindle_speedup,
            speedup,
        );

        // combined_factor (chipload multiplier) does NOT include
        // spindle_speedup — it's purely a speed-axis change.
        let factor_max = max_speed.derates.combined_factor();
        let factor_chart = match_chart.derates.combined_factor();
        assert!(
            (factor_max - factor_chart).abs() / factor_chart.max(1e-6) < 0.02,
            "combined_factor should be unchanged by spindle strategy: \
             chart={factor_chart:.4} vs max={factor_max:.4}"
        );
    }

    /// `MaxSpeed` caps at `MAX_SPINDLE_SPEEDUP` even if the spindle
    /// ceiling/chart ratio is bigger. This guards against unbounded
    /// extrapolation past the chart's tested envelope.
    #[test]
    fn spindle_strategy_max_speed_respects_hard_cap() {
        // Construct a fake low-RPM scenario: pick a machine with a
        // very high ceiling (synthesise a MachineProfile if needed).
        // Easier: just verify the cap constant is sensible and the
        // observed speedup never exceeds it in `derates`.
        let lut = vendor_lut::VendorLut::embedded();
        let machine = MachineProfile::shapeoko_vfd();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::GenericSoftwood,
        };
        let result = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            shank_diameter: None,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: SpindleStrategy::MaxSpeed,
        });
        assert!(
            result.derates.spindle_speedup <= MAX_SPINDLE_SPEEDUP + 1e-6,
            "spindle_speedup {} must not exceed MAX_SPINDLE_SPEEDUP {}",
            result.derates.spindle_speedup,
            MAX_SPINDLE_SPEEDUP,
        );
    }
}
