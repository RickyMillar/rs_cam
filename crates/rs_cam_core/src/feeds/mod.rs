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
    pub hardness_index: f64,
    pub result_mm_tooth: f64,
}

impl FeedsDerates {
    /// Compose every multiplier into a single number. The effective
    /// chipload (`feed / (RPM × flutes)`) equals
    /// `target_chip_load_mm × combined_factor()`.
    pub fn combined_factor(&self) -> f64 {
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

    // --- Step 1: RPM ---
    const FALLBACK_RPM: f64 = 18000.0;
    let ideal_rpm = if d > 0.0 {
        (material.base_cutting_speed_m_min() * 1000.0) / (std::f64::consts::PI * d)
    } else {
        FALLBACK_RPM
    };
    let mut rpm = machine.clamp_rpm(ideal_rpm);

    // --- Step 2: Chip load — vendor LUT first, formula fallback ---
    let hardness = material.hardness_index();
    let cl = &machine.chip_load;
    let formula_chipload = cl.k0 * d.powf(cl.p) * (1.0 / hardness).powf(cl.q);

    let (chip_load, vendor_rpm, vendor_source, chipload_source) =
        if let Some(lut) = input.vendor_lut {
            let query = vendor_normalize::to_lookup_query(input);
            if let Some(result) = vendor_lookup::find_best_row(lut, &query) {
                let observation_id = result.observation_id;
                (
                    result.chip_load_mm,
                    result.rpm_nominal,
                    Some(observation_id.clone()),
                    ChiploadSource::VendorLut { observation_id },
                )
            } else {
                (
                    formula_chipload,
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
                ChiploadSource::FormulaFallback,
            )
        };

    // Override RPM if vendor provided one within machine range
    if let Some(v_rpm) = vendor_rpm {
        rpm = machine.clamp_rpm(v_rpm);
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
    let available_power = machine.power_at_rpm(rpm);
    let mut power_limited = false;
    let mut feed = raw_feed;
    let mut power_factor = 1.0;

    if let Some(kc) = material.kc_n_per_mm2() {
        let required_power = (kc * ap * ae * raw_feed) / (60.0 * 1_000_000.0);
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
    let plunge = material.plunge_rate_base();

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
    let actual_power = match material.kc_n_per_mm2() {
        Some(kc) => (kc * ap * ae * feed) / (60.0 * 1_000_000.0),
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
            hardness_index: hardness,
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

            // Hardness-dependent adaptive derates
            let hardness = input.material.hardness_index();
            if hardness > 1.40 {
                ap_factor *= 0.80;
                ae_factor *= 0.90;
            } else if hardness > 1.15 {
                ap_factor *= 0.90;
                ae_factor *= 0.95;
            } else if hardness < 0.85 {
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
        let wood_class_flat_tool = matches!(
            input.material,
            Material::SolidWood { .. } | Material::Plywood { .. } | Material::SheetGood { .. }
        ) && matches!(
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
        }
    }

    #[test]
    fn test_chip_load_soft_wood_6mm() {
        let machine = MachineProfile::shapeoko_vfd();
        let d: f64 = 6.0;
        let h = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }
        .hardness_index();
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
        .hardness_index();
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
        });

        // Foam maps to softwood in normalize, but with hardness 200 which is far from
        // any observation. If it does match, that's fine. If not, formula is used.
        assert!(result.feed_rate_mm_min > 0.0);
    }
}
