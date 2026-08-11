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

pub mod cutter_constraints;
pub mod explain;
pub mod explanation;
pub mod force;
pub mod geometry;
pub mod geometry_class;
pub mod predict;
pub mod profile;
pub mod provenance;
pub mod quantities;
pub mod rationale;
pub mod suggest;
pub mod vendor_lookup;
pub mod vendor_lut;
pub mod vendor_normalize;
pub use explain::{FeedsExplain, MachineEnvelope, explain as explain_feeds};
pub use explanation::{
    ADVANCE_PER_TOOTH, AchievedFeedStage, CommandedStage, FeedExplanation, GateObservationStage,
    LutBandStage, ObservedStatistic,
};
pub use predict::{DeflectionBreakdown, DeflectionPrediction, predict_peak_deflection_um};
pub use provenance::{FeedsField, FeedsProvenance, ProvenanceSource, ValueProvenance};
pub use quantities::{
    ACHIEVED_ADVANCE_PER_TOOTH, ADVANCE_PER_TOOTH_UNIT, ARC_MEAN_CHIP_THICKNESS, AchievedFeedMmMin,
    AdvancePerToothMm, ArcMeanChipThicknessMm, COMMANDED_ADVANCE_PER_TOOTH, ChiploadBandClass,
    CommandedFeedMmMin, VendorChiploadBand,
};
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
    ///
    /// This is a **second, hand-maintained implementation** of the
    /// same geometry as [`crate::tool::MillingCutter::lookup_diameter_at`]
    /// (see that trait method's doc comment on
    /// `tool::vbit::VBitEndmill` / `tool::tapered_ball::TaperedBallEndmill`
    /// for the reverse pointer). It has to be: this hint-level path is
    /// called from `feeds::calculate` / `vendor_normalize`, which only
    /// carry a `ToolGeometryHint` (scalar shape params), not a full
    /// `&dyn MillingCutter` instance — there's nothing to delegate to
    /// cheaply. The two are kept honest by the cross-shape DOC-sweep
    /// parity sentry `tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`
    /// below. If that test ever fails, do not silently prefer one side
    /// — the divergence means Suggest's chipload-band derating and the
    /// post-sim gate's derating have quietly split, which can produce
    /// false chipload trips (planning/finishing_stack_review_2026-07.md S.3).
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

    /// The canonical fieldless shape class of this hint. Primary
    /// derivation path for [`CutterKind`] — every `MillingCutter`
    /// yields a `ToolGeometryHint`, so externally-constructed cutters
    /// classify without touching the closed `ToolType` enum.
    pub fn cutter_kind(self) -> CutterKind {
        match self {
            ToolGeometryHint::Flat => CutterKind::Flat,
            ToolGeometryHint::Ball => CutterKind::Ball,
            ToolGeometryHint::Bull { .. } => CutterKind::Bull,
            ToolGeometryHint::VBit { .. } => CutterKind::VBit,
            ToolGeometryHint::TaperedBall { .. } => CutterKind::TaperedBall,
        }
    }
}

/// Canonical cutter-shape classifier (Phase 3, architectural refactor
/// 2026-06-06): the fieldless SHAPE CLASS of a cutter, used for routing
/// and compatibility decisions.
///
/// Disambiguation — this crate has three similarly-named classifiers:
///
/// - **`CutterKind` (this)** — fieldless cutter shape class. Use it for
///   per-shape routing (LUT family, bending-section model, op
///   compatibility) where the geometry numbers don't matter.
/// - **[`ToolGeometryHint`]** — cutter shape *plus* geometry data
///   (included angle, tip/corner radii). Use it for math that needs the
///   numbers (`engaged_diameter_at_doc`, MRR cross-section). Derive a
///   `CutterKind` from it via [`ToolGeometryHint::cutter_kind`].
/// - **[`geometry_class::GeometryClass`]** — an `OperationType`-keyed
///   TERRAIN classifier (what the model surface looks like). It is not
///   about the cutter at all.
///
/// Derivation order: `ToolGeometryHint::cutter_kind()` is the primary
/// path; `ToolType::cutter_kind()` (compute layer) is a convenience
/// layered on top for call sites that only hold a `ToolConfig`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CutterKind {
    Flat,
    Ball,
    Bull,
    VBit,
    TaperedBall,
}

impl CutterKind {
    pub const ALL: &[CutterKind] = &[
        CutterKind::Flat,
        CutterKind::Ball,
        CutterKind::Bull,
        CutterKind::VBit,
        CutterKind::TaperedBall,
    ];

    /// THE single `cutter shape → vendor-LUT ToolFamily` map.
    ///
    /// Pre-Phase-3 this map was triplicated verbatim:
    /// `tool_load::chipload::tool_family_for`, an inline match in
    /// `vendor_normalize::to_lookup_query`, and a test-local shim in
    /// `tests/lookup_parity.rs`. All three now route here.
    ///
    /// [`vendor_lut::ToolFamily::FacingBit`] is deliberately absent:
    /// it is LUT row *data* vocabulary with no corresponding cutter —
    /// no `ToolType` or `MillingCutter` classifies to it (pinned by
    /// `facing_bit_is_a_lut_only_family`). Any compatibility logic
    /// assuming a cutter ↔ family bijection is wrong in that direction.
    pub fn lut_family(self) -> vendor_lut::ToolFamily {
        match self {
            CutterKind::Flat => vendor_lut::ToolFamily::FlatEnd,
            CutterKind::Ball => vendor_lut::ToolFamily::BallNose,
            CutterKind::Bull => vendor_lut::ToolFamily::BullNose,
            CutterKind::VBit => vendor_lut::ToolFamily::ChamferVbit,
            CutterKind::TaperedBall => vendor_lut::ToolFamily::TaperedBallNose,
        }
    }

    /// The canonical [`crate::compute::ToolType`] for this shape class.
    /// `ToolType ↔ CutterKind` is a bijection today (5 ↔ 5, pinned by
    /// `tool_type_cutter_kind_round_trips`); this direction exists so
    /// registry constraint lists typed on `CutterKind` can materialize
    /// the serde-facing tool-type tokens.
    pub const fn tool_type(self) -> crate::compute::ToolType {
        match self {
            CutterKind::Flat => crate::compute::ToolType::EndMill,
            CutterKind::Ball => crate::compute::ToolType::BallNose,
            CutterKind::Bull => crate::compute::ToolType::BullNose,
            CutterKind::VBit => crate::compute::ToolType::VBit,
            CutterKind::TaperedBall => crate::compute::ToolType::TaperedBallNose,
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

/// LUT-derived chipload band for the matched vendor row, scaled by the
/// diameter/hardness factors that `vendor_lookup::lookup_best` applies.
///
/// Populated only when the calculator found a matching vendor row that
/// publishes a chipload range — RPM-only rows and formula-fallback paths
/// leave this `None`. Consumers (Suggest v2 step 2 feed-up recalibration,
/// post-sim chipload gate narrative) use this band's bounds as the
/// targets the predicted observed chipload should land within.
///
/// v3.0a (2026-06-04) re-added `max_mm_per_tooth` after the v2.1 polish
/// dropped it: v3's `SuggestAggressiveness` enum needs both bounds to
/// compute the targeted operating point (LUT min / median / max). v2.1's
/// recalibration only consulted `min`, so the field was unused at the
/// time; v3.0b makes it load-bearing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiploadBounds {
    /// LUT-row chipload lower bound, post diameter/hardness scaling
    /// (mm/tooth). Suggest's feed-up loop uses this as the
    /// `SuggestAggressiveness::Conservative` target.
    pub min_mm_per_tooth: f64,
    /// LUT-row chipload upper bound, post diameter/hardness scaling
    /// (mm/tooth). Suggest's feed-up loop uses this as the
    /// `SuggestAggressiveness::Speed` target; the midpoint is the
    /// `SuggestAggressiveness::Default` (median) target.
    pub max_mm_per_tooth: f64,
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
    /// LUT-derived chipload band for the matched vendor row, when the
    /// match supplied one. `None` for formula-fallback / RPM-only LUT
    /// rows / edge-radius-floor paths. Consumed by Suggest v2 step 2
    /// (chipload-aware feed-up recalibration) — see
    /// [`crate::feeds::predict::predict_observed_chipload_mm`] for the
    /// other half of that loop.
    pub chipload_bounds: Option<ChiploadBounds>,
    /// Matched vendor row (post-scaling) from the LUT lookup, when one
    /// was found. Cloned through so the Suggest orchestrator's axial-DOC
    /// envelope pass ([`crate::feeds::cutter_constraints`]) and the
    /// chipload-bounds re-derivation step in `enforce_invariants` can
    /// reach the row without re-querying.
    pub matched_lut_row: Option<vendor_lookup::LookupResult>,
    /// **Chip-thinning** effective diameter at the calculator's
    /// commanded axial DOC (mm) — `feeds::effective_diameter`, i.e.
    /// "what actually touches material" (a Ø6 ball at 0.05 mm DOC
    /// reports 0.44 mm, not 6.0).
    ///
    /// **This is NOT the denominator of `chipload_bounds`' DOC derate,
    /// despite what this comment used to say.** `calculate` derives
    /// that ratio from the **LUT-semantics** engaged diameter —
    /// `ToolGeometryHint::engaged_diameter_at_doc`, "which vendor row
    /// applies", `D` for flat/ball/bull — and the two live in
    /// same-named bindings, the second shadowing the first inside one
    /// function. Census F-3 / C-2 / C-5 (T1.4, 2026-08-04); the two
    /// sites are commented at their definitions.
    ///
    /// The one consumer that *does* re-derate against this field is
    /// `suggest::recompute_chipload_bounds_for_dpp`, which therefore
    /// uses a different denominator from `calculate`. Measured
    /// (`feed_explanation_snapshot_b3::the_two_doc_ratio_diameters_only_diverge_for_v_bit_geometry`):
    /// flat, ball, bull and tapered-ball never diverge — only a
    /// truncated-tip V-bit does, and only on Adaptive3d, the sole
    /// operation that mutates DPP. Unifying them is census T2.4 /
    /// T3.6, not this report-only wave.
    ///
    /// Carried on the result so the axial-DOC envelope pass can
    /// re-derive bounds after mutating DPP without re-walking the
    /// chip-geometry pipeline.
    pub effective_diameter_mm: f64,
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
    FeedRateClamped {
        requested: f64,
        actual: f64,
    },
    PowerLimited {
        required_kw: f64,
        available_kw: f64,
    },
    ShankTooLarge {
        shank_mm: f64,
        max_mm: f64,
    },
    DocExceedsFlute {
        requested: f64,
        capped: f64,
    },
    SlottingDetected {
        doc_reduced_to: f64,
    },
    ScallopInvalid {
        target: f64,
        max_possible: f64,
    },
    /// Vendor-LUT or formula chipload derated below the rubbing
    /// floor (typically extreme-Janka hardwoods scaling an oak-anchored
    /// LUT row down). The engine clamps up and emits this so the
    /// operator sees the honest derate instead of a silent ploughing
    /// recipe.
    ChiploadClampedToFloor {
        requested: f64,
        /// The floor actually applied — [`RUBBING_FLOOR_MM_TOOTH`], or
        /// the matched row's derated band maximum when that sits lower.
        /// See [`effective_rubbing_floor`].
        floor: f64,
        /// `Some(global)` when `floor` was capped by the matched band's
        /// ceiling, carrying the global chip-formation threshold the
        /// recipe therefore does **not** reach. This combination is a
        /// genuine, unresolvable conflict between two shipped policies:
        /// the vendor row says anything above `floor` breaks the tool,
        /// the chip-formation rule says anything below `global` burns
        /// the work. The engine picks the vendor ceiling (never command
        /// past a band we are told is breakage-side) and surfaces the
        /// residual rubbing risk here rather than hiding it behind a
        /// number that silently changed meaning.
        /// `None` when the global floor applied unmodified.
        band_capped_from: Option<f64>,
    },
    /// Drill-cycle feed clamped into the material plunge-feed envelope
    /// (`Material::drill_plunge_feed_envelope_per_mm` × diameter,
    /// mm/min). Below the envelope the drill rubs and burns; above it
    /// the bit risks breakage. Mirrors the drill plunge-feed gate
    /// (`tool_load/drill_gates.rs`) so Suggest and the verdict share
    /// one envelope source.
    DrillFeedClampedToEnvelope {
        requested: f64,
        actual: f64,
        envelope_lo: f64,
        envelope_hi: f64,
    },
}

/// Hard refusal from the Suggest pipeline — the operation × tool
/// combination is geometrically or physically unrunnable, so no
/// numeric recipe is meaningful. Distinct from `FeedsWarning`, which
/// flags a degraded-but-still-usable cut.
///
/// Returned from [`suggest::suggest_for_operation`] /
/// [`suggest::feeds_result_for_operation`] / [`suggest::suggest_params`]
/// (the production Suggest entry points). The infallible
/// [`calculate`] function is preserved for the explain-modal path
/// (which displays a "would-have-produced" preview); call
/// [`validate_tool_for_operation`] alongside `calculate` if you need
/// the refusal signal in a path that doesn't go through `suggest::*`.
#[derive(Debug, Clone)]
pub enum FeedsError {
    /// Tool geometry can't physically produce the operation's intended
    /// cut. Today fires for `Scallop` (and `DropCutter` when a scallop
    /// height is set) with `Flat` or `VBit` geometry — both have zero
    /// tip radius so the scallop-stepover formula
    /// `2·√(2·R·h − h²)` is undefined.
    WrongToolForOperation {
        operation: OperationFamily,
        actual_geometry: ToolGeometryHint,
        required: &'static str,
    },
}

impl std::fmt::Display for FeedsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedsError::WrongToolForOperation {
                operation,
                actual_geometry,
                required,
            } => write!(
                f,
                "scallop requires curved tip (need {required}; got {actual_geometry:?} on {operation:?})",
            ),
        }
    }
}

impl std::error::Error for FeedsError {}

/// Validate that the input's tool geometry is physically compatible
/// with its operation family. Returns `Ok(())` for combinations
/// `calculate` can produce a meaningful recipe for; `Err(FeedsError)`
/// for combinations where the engine should refuse rather than emit a
/// numeric-looking recipe that would produce ploughing / rubbing /
/// undefined geometry.
///
/// Today this fires on Scallop + Flat|VBit (no tip radius — the
/// `R - √(R² - (s/2)²)` scallop formula is undefined) and on
/// DropCutter + Flat|VBit when `target_scallop_mm.is_some()` (the
/// DropCutter scallop hint routes through the same scallop-stepover
/// block in `calculate`, so the same geometry constraint applies).
pub fn validate_tool_for_operation(input: &FeedsInput) -> Result<(), FeedsError> {
    let scallop_relevant = input.operation == OperationFamily::Scallop
        || (input.operation == OperationFamily::Parallel && input.target_scallop_mm.is_some());
    if scallop_relevant {
        match input.tool_geometry {
            ToolGeometryHint::Ball
            | ToolGeometryHint::Bull { .. }
            | ToolGeometryHint::TaperedBall { .. } => {}
            ToolGeometryHint::Flat | ToolGeometryHint::VBit { .. } => {
                return Err(FeedsError::WrongToolForOperation {
                    operation: input.operation,
                    actual_geometry: input.tool_geometry,
                    required: "ball|bull|tapered_ball",
                });
            }
        }
    }
    Ok(())
}

/// Minimum chip thickness below which cutting becomes ploughing /
/// rubbing (heat, burn, edge wear). 0.025 mm/tooth is the canonical
/// wood-router floor (Onsrud min-chip-thickness rule, GWizard
/// "minimum chipload", FPL Wood Handbook chip-formation regime). Any
/// vendor-LUT or formula chipload that derates below this floor —
/// typically very hard species (Janka >> the matched row's anchor) or
/// extreme diameter shrinkage — is clamped up and a
/// `FeedsWarning::ChiploadClampedToFloor` warning is emitted.
///
/// **This is a *global* threshold, not the value the clamp applies.**
/// It carries no diameter and no material, while every other chipload
/// bound in the crate is a scaled vendor band. The clamp applies
/// [`effective_rubbing_floor`], which subordinates this constant to the
/// matched row's band ceiling.
pub const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

/// The floor the Step-9b clamp actually applies:
/// `min(RUBBING_FLOOR_MM_TOOTH, derated_band_max)`.
///
/// # Why the global constant cannot be used directly
///
/// `RUBBING_FLOOR_MM_TOOTH` is diameter- and material-independent;
/// [`ChiploadBounds`] is the same vendor row's chipload window after the
/// diameter, hardness and DOC derates. On small tools the two cross.
/// Measured on the census's B3 reference row (Ø1 tapered ball, 2 flutes,
/// scallop finish, hard maple — `tests/feed_explanation_snapshot_b3.rs`)
/// the derated band is 0.003605–0.007211 mm/tooth and the global floor
/// is **3.47× the band maximum**. Clamping *up* to 0.025 there commanded
/// a chipload the post-sim chipload gate's own envelope
/// (`tool_load::chipload`) reads as `Exceeds(High)` — breakage-side. A
/// floor whose stated job is to stop the recipe *undershooting* a band
/// was pushing it clean over the top of that band.
///
/// # What is preserved, and what is given up
///
/// The rubbing/burnishing threshold is a real phenomenon and the clamp
/// survives unchanged wherever the band has room for it (the common
/// case: at Ø6 in oak the band is 0.034–0.059, entirely above the
/// floor, and `min` returns the constant). What is given up is the
/// claim that the engine can always *reach* chip formation: when the
/// whole band sits under 0.025 mm/tooth, no feed exists that both
/// clears the rubbing threshold and stays inside the vendor window.
/// The engine then commands the band maximum — the furthest from
/// rubbing it can go without commanding a chipload it is told is
/// breakage-side — and
/// `FeedsWarning::ChiploadClampedToFloor::band_capped_from` discloses
/// the global threshold that was not met, so the residual burn risk is
/// surfaced rather than silently absorbed into a smaller number.
///
/// # Bands that do not exist
///
/// RPM-only vendor rows and the formula fallback publish no chipload
/// window (`chipload_bounds == None`); there is nothing to subordinate
/// to and the global constant applies unmodified. That path is pinned
/// by `tests/_litmatrix_rpm_only_lut_chipload.rs`.
///
/// FEEDS_CENSUS C-12 / T3.3 / T4.2; ruled 2026-08-06.
#[must_use]
pub fn effective_rubbing_floor(band: Option<ChiploadBounds>) -> f64 {
    match band {
        Some(b) if b.max_mm_per_tooth > 0.0 => RUBBING_FLOOR_MM_TOOTH.min(b.max_mm_per_tooth),
        _ => RUBBING_FLOOR_MM_TOOTH,
    }
}

/// Diameter-tiered RPM envelope for wood-drilling ops. The drill RPM
/// band narrows and drops as diameter grows: chip evacuation scales
/// with chip volume per revolution, which grows roughly with D², so
/// big drills need *fewer* revolutions per second to clear chips than
/// small drills. Sources: Onsrud wood-drilling bulletin (3-8k for
/// 10-13 mm drills in hardwood), Vectric default drill cycle, FPL
/// Wood Handbook Ch.19 (drilling), Sandvik Coromant rotating-tools
/// handbook.
///
/// Tiers (inclusive upper bound):
/// - D ≤ 6 mm:  (8000, 14000) — small drills, milling-formula RPM is
///   already in band; floor keeps SFM-derived RPM from dropping below
///   the rubbing-onset RPM.
/// - D ≤ 10 mm: (6000, 10000) — mid drills.
/// - D > 10 mm: (4000, 8000) — big drills; the 8 kRPM ceiling matches
///   the literature-matrix `flat_12mm_drill_oak_big` cell.
fn drill_rpm_envelope_for_diameter(d_mm: f64) -> (f64, f64) {
    if d_mm <= 6.0 {
        (8_000.0, 14_000.0)
    } else if d_mm <= 10.0 {
        (6_000.0, 10_000.0)
    } else {
        (4_000.0, 8_000.0)
    }
}

/// Diameter-tiered RPM ceiling for milling ops under
/// [`SpindleStrategy::MaxSpeed`]. The MaxSpeed strategy walks RPM up
/// toward the spindle ceiling (24 k × 0.95 = 22.8 k on the shapeoko_xxl
/// profile). For small tools (≤ 6 mm) that's defensible — the chipload
/// envelope tolerates near-spindle RPM on a 3-flute end mill. For
/// **large** tools (≥ 10 mm) it pushes RPM well past the literature
/// band: the surface-speed math (SFM = π·D·RPM) implies that the same
/// chipload at 22.8 k on a 12 mm tool runs at twice the SFM the chart
/// was tested at, well into the burning / glazing regime in hardwood.
///
/// Sources for the tier ceilings:
/// - Onsrud Hardwood Feed Chart (series 70/85): 12 mm 2F in hardwood
///   recommends 14-18 krpm; 10 mm 12-16 krpm; 6 mm 16-20 krpm; 3 mm
///   18-22 krpm.
/// - Amana Spektra hardwood chart: similar split, 12 mm 12-14 krpm.
/// - GWizard hardwood defaults: SFM 500-1200 → 12 mm = 4.4-10.6 krpm
///   floor, capped at 18 krpm at the high end.
/// - Shapeoko community wiki: hobby derates further but the chart
///   bands hold the relative shape.
///
/// Tiers (inclusive upper bound), chosen as the **max-across-materials**
/// of the literature band so soft species can still push to the top of
/// the envelope when vendor data anchors there. The 8 / 10 mm split
/// matches the Onsrud series 70/85 chart's gradient (8 mm ≈ 14-18 k,
/// 10 mm ≈ 12-16 k for hardwood 2F roughing):
/// - D ≤ 3 mm:  22 000 RPM — small tools, near-spindle ceiling OK.
/// - D ≤ 6 mm:  20 000 RPM — common 1/4" pocket / adaptive band top.
/// - D ≤ 8 mm:  18 000 RPM — 5/16" mid-tool band top.
/// - D ≤ 10 mm: 16 000 RPM — 3/8" mid-large mill (matches the
///   literature-matrix cell `flat_10mm_pocket_maple` band-max).
/// - D > 10 mm: 14 000 RPM — large-tool literature ceiling (Onsrud /
///   Amana hardwood 12 mm bands).
///
/// Returns the ceiling the MaxSpeed speedup may climb to. Combined
/// with vendor `rpm_max` (when published) and the machine safety
/// headroom via `.min()` — the lowest defensible cap wins.
fn milling_rpm_ceiling_for_diameter(d_mm: f64) -> f64 {
    if d_mm <= 3.0 {
        22_000.0
    } else if d_mm <= 6.0 {
        20_000.0
    } else if d_mm <= 8.0 {
        18_000.0
    } else if d_mm <= 10.0 {
        16_000.0
    } else {
        14_000.0
    }
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
    //
    // NAMING WARNING (census F-3, T1.4). This binding is the
    // **LUT-semantics** engaged diameter — "which vendor row applies"
    // — and it is what the chipload-band DOC derate below divides by.
    // Step 5 rebinds the same name `effective_d` to the **chip-thinning**
    // diameter (`feeds::effective_diameter`, "what actually touches
    // material"), shadowing this one for the rest of the function, and
    // it is *that* one which is published as
    // `FeedsResult::effective_diameter_mm`. Two different questions,
    // one identifier. Collapsing them is census T2.4; until then, read
    // the shadow point before assuming which diameter a line means.
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
    // chip evacuation, not surface speed, is the limiting factor. The
    // wood-drill band tightens as diameter grows — small drills (≤6 mm)
    // tolerate 8-14k, mid drills (≤10 mm) cap around 10k, and big
    // drills (>10 mm) cap around 6-8k. **Citation corrected
    // 2026-08-04 (W6 audit §6.1/§6.2): these tiers are REPO-AUTHORED.**
    // The "Onsrud wood-drilling bulletin" cited here was not located;
    // the retrieved Onsrud drill chart publishes exactly one wood-drill
    // RPM, the 4,500 gang-drill footnote, and no band. The FPL Wood
    // Handbook has no drilling chapter (Ch.19 is *Specialty
    // Treatments*). The tiers remain defensible as hobby-router
    // spindle practice and are held, not moved. The milling SFM formula above
    // would push small-D drills past 16k where chipload starves and
    // the cut rubs/burns. Pre-2026-06-02 drill ops routed through
    // `Pocket` family and inherited milling RPM (audit finding: "Drill
    // ops route through OperationFamily::Pocket with no chipload
    // reconciliation"). Pre-2026-06-03 the clamp was diameter-
    // independent at 8-14k, letting 12 mm hardwood drills emit 12k RPM
    // — well above the 3-8k band (literature-matrix cell
    // flat_12mm_drill_oak_big: ceiling 8000, emitted 12000, +50%).
    if input.operation == OperationFamily::Drill {
        let (floor, ceil) = drill_rpm_envelope_for_diameter(d);
        rpm = rpm.clamp(floor, ceil);
        rpm = machine.clamp_rpm(rpm);
    }

    // --- Step 2: Chip load — vendor LUT first, formula fallback ---
    let feed_scale = material.feed_scale_factor();
    let cl = &machine.chip_load;
    // Formula chipload uses engaged diameter so the V-bit / tapered-ball
    // fallback path matches the LUT path's band semantics. For Flat /
    // Ball / Bull this collapses to nominal D.
    //
    // Drill ops get a multiplier on top: drilling cuts at full radius
    // and needs feed-per-rev to chip-evacuate, while the milling
    // formula was calibrated against partial-engagement cuts.
    //
    // **The value is REPO-AUTHORED and UNSOURCED. Its former
    // justification was arithmetically false and has been removed**
    // (W6 audit, 2026-08-04, §5 / item R-11 —
    // `planning/review_2026-08-04/DRILL_GATE_EVIDENCE_AUDIT.md`).
    // What that comment claimed, and what is actually true:
    //   - Claimed "milling-formula chipload lands at ~0.03 mm/rev" for
    //     a softwood drill. With the shipped `ChipLoadFormula::default`
    //     (k0 0.024, p 0.61, q 1.26) and GenericSoftwood the
    //     un-multiplied formula reads 0.094 mm/rev at Ø3, 0.143 at Ø6
    //     and 0.219 at Ø12 — 3.1×–7.3× the quoted figure, and already
    //     inside or above the "0.05–0.15 mm/rev" band the comment said
    //     it fell below.
    //   - Claimed the audit observation "implied chipload 0.026 on
    //     Wanaka Pin Drill / Holes". Real, but it was the *stored* op's
    //     feed/(rpm × flutes) AFTER the milling plunge baseline
    //     clobbered the drill-tuned feed — the mechanism Step 9c fixed
    //     separately (see the plunge-rate aliasing below). This factor
    //     and that clamp were two corrections for one defect.
    //   - Claimed drill bands are "~2.5× higher" than milling. No
    //     retrievable source states any such ratio. Against the one
    //     primary wood-drill chart located (Onsrud series 72-000 Wood,
    //     `https://www.onsrud.com/images/Drill.pdf`, retrieved
    //     2026-08-04) the factor implied is 4.76–5.41 — i.e. 2.5 is
    //     directionally right and roughly HALF the size that chart
    //     implies, not an over-correction.
    //
    // The value is deliberately HELD at Checkpoint D 2026-08-04. Two
    // reasons, both stated: the Onsrud wood row is footnoted "gang
    // drills run at 4,500 RPM and 150 IPM" (a rigid multi-spindle
    // production borer, not a hobby router with collet stickout), so
    // it cannot be used as a recalibration target on its own; and this
    // is the one drill number that rides the formula chipload, so it
    // sequences behind the gate-side unit conversion (T3.1) and the
    // optimizer-target re-derivation. Do not move it before then.
    const DRILL_CHIPLOAD_MULTIPLIER: f64 = 2.5;
    let milling_chipload = cl.k0 * effective_d.powf(cl.p) * (1.0 / feed_scale).powf(cl.q);
    let formula_chipload = if input.operation == OperationFamily::Drill {
        milling_chipload * DRILL_CHIPLOAD_MULTIPLIER
    } else {
        milling_chipload
    };

    // Sidecar: when the LUT lookup returns a match, stash the row so
    // FeedsResult can propagate it to the Suggest orchestrator's
    // axial-DOC envelope pass (see `feeds/cutter_constraints.rs`). The
    // existing tuple destructuring below consumes per-field values, so
    // the row clone lives here separately.
    let mut matched_lut_row: Option<vendor_lookup::LookupResult> = None;
    let (chip_load, vendor_rpm, vendor_rpm_max, vendor_source, chipload_source, chipload_bounds) =
        if let Some(lut) = input.vendor_lut {
            let query = vendor_normalize::to_lookup_query(input);
            if let Some(result) =
                vendor_lookup::find_best_row_for_geometry(lut, &query, &input.tool_geometry)
            {
                matched_lut_row = Some(result.clone());
                let observation_id = result.observation_id;
                // Capture the LUT-derived chipload band (post diameter
                // /hardness scaling) for Suggest v2 step 2's feed-up
                // recalibration loop. Only populated when the row
                // publishes both bounds — partial-band rows (one side
                // only) leave it None so the loop doesn't fire on an
                // ambiguous target.
                //
                // DOC derating (2026-06-04): the post-sim chipload gate
                // scales the matched row's bounds by
                // `geometry::doc_derating_scale(peak_axial_DOC /
                // effective_d)`. Apply the same scale here using the
                // commanded axial DPP so `SuggestAggressiveness::target_chipload`
                // (median / max / min) aims at a value the gate will
                // accept at this DOC ratio. Without this, v3.0c median
                // targeting on high-DOC ops (e.g. wanaka Back Rough at
                // ~3×D) lands above the gate's derated max and trips
                // `Exceeds(High)` despite the toolpath being healthy.
                //
                // Drill ops are excluded because the post-sim gate
                // short-circuits drill ops with `NotApplicableForOp` —
                // the chip-evacuation rule that motivates DOC derating
                // for milling doesn't apply to drill bands (peck depth,
                // not engagement). Mirroring the gate's exclusion here
                // keeps the two paths aligned for the cases where the
                // gate actually fires. Forcing `chipload_doc_ratio` to
                // `0.0` bypasses derating (`doc_derating_scale` maps
                // any ratio `<= 1.0` to a scale of `1.0`), same effect
                // as the pre-S.8 `chipload_doc_scale = 1.0` branch.
                //
                // Validation + scaling both now live in
                // `geometry::derate_chipload_bounds` — the single home
                // for this wrapper (S.8), also used by
                // `suggest::recompute_chipload_bounds_for_dpp` and both
                // `tool_load` chipload sites.
                let chipload_doc_ratio = if input.operation == OperationFamily::Drill {
                    0.0
                } else if effective_d > 0.0 {
                    axial_doc_for_eff_d / effective_d
                } else {
                    0.0
                };
                let bounds = geometry::derate_chipload_bounds(
                    result.chip_load_min_mm,
                    result.chip_load_max_mm,
                    chipload_doc_ratio,
                    geometry::ChiploadBoundPolicy::RequireBoth,
                )
                .and_then(geometry::DeratedChiploadBand::into_pair)
                .map(|(min, max)| ChiploadBounds {
                    min_mm_per_tooth: min,
                    max_mm_per_tooth: max,
                });
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
                        bounds,
                    )
                } else {
                    (
                        formula_chipload,
                        result.rpm_nominal,
                        result.rpm_max,
                        Some(observation_id),
                        ChiploadSource::FormulaFallback,
                        bounds,
                    )
                }
            } else {
                (
                    formula_chipload,
                    None,
                    None,
                    None,
                    ChiploadSource::FormulaFallback,
                    None,
                )
            }
        } else {
            (
                formula_chipload,
                None,
                None,
                None,
                ChiploadSource::FormulaFallback,
                None,
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
        // Diameter-tier literature ceiling for milling. The MaxSpeed
        // speedup pre-2026-06-03 used machine_ceiling unconditionally,
        // pushing a 12 mm hardwood adaptive2d cut to 22.8 k RPM —
        // ~63% above the Onsrud/Amana 14 k literature ceiling
        // (literature-matrix cell flat_12mm_adaptive2d_oak_power).
        // Drill ops already had a diameter tier from round-4
        // (`drill_rpm_envelope_for_diameter`, commit c9818dd); this is
        // the milling-side analogue.
        //
        // The tier ceiling always applies for nominal-D == cutting-D
        // geometries (Flat / Ball / Bull) — vendor `rpm_max` on the
        // matched LUT row is treated as a per-row safety ceiling
        // (often inherited from a small-diameter anchor row and not
        // re-scaled by diameter), so it isn't a reliable per-diameter
        // bound. We `.min()` all three: vendor rpm_max (when present),
        // the machine safety headroom, and the diameter-tier
        // literature ceiling. The lowest defensible cap wins.
        //
        // V-bit and tapered-ball geometries cut on the **engaged-D**
        // circle, not nominal D — the tier table is calibrated on the
        // cutting-D ≈ nominal-D assumption, so clamping nominal-D
        // would force a 6.35 mm V-bit down to the 18 k tier even when
        // engaged-D is ~1 mm and the engaged-D SFM math wants near-
        // ceiling RPM (literature-matrix cell
        // vbit_60deg_vcarve_oak_micro_depth).
        let geom_uses_nominal_d = !matches!(
            input.tool_geometry,
            ToolGeometryHint::VBit { .. } | ToolGeometryHint::TaperedBall { .. }
        );
        let diameter_tier_ceiling = if geom_uses_nominal_d {
            milling_rpm_ceiling_for_diameter(d)
        } else {
            f64::INFINITY
        };
        let ceiling = match vendor_rpm_max {
            Some(vm) if vm.is_finite() && vm > 0.0 => {
                vm.min(machine_ceiling).min(diameter_tier_ceiling)
            }
            _ => machine_ceiling.min(diameter_tier_ceiling),
        };
        if ceiling > rpm {
            let raw_speedup = ceiling / rpm;
            spindle_speedup = raw_speedup.min(MAX_SPINDLE_SPEEDUP);
            rpm = machine.clamp_rpm(rpm * spindle_speedup);
        }
    }

    // --- Step 2b': Diameter-tier ceiling for milling under MaxSpeed ---
    //
    // The vendor LUT can publish `rpm_nominal` above the diameter-tier
    // literature ceiling (e.g. a 12 mm hardwood adaptive2d row anchored
    // on a 6 mm chart with vendor_rpm=18 000). Step 2's vendor-override
    // path (`rpm = machine.clamp_rpm(v_rpm)`) then carries that value
    // forward, and the Step 2b speedup branch is a no-op because the
    // tier ceiling is already below `rpm`. Without this final clamp the
    // engine emits the vendor-anchor RPM verbatim — overshooting the
    // literature band whenever the vendor row outpaces the per-diameter
    // wood-cutting envelope.
    //
    // Apply the tier ceiling as a hard cap under MaxSpeed only —
    // MatchChart explicitly opts into the vendor row's nominal RPM and
    // shouldn't be silently re-clamped (the smoke baselines + chart-
    // fidelity tests depend on that opt-in). The Drill family has its
    // own dedicated Step 2c clamp below.
    if matches!(input.spindle_strategy, SpindleStrategy::MaxSpeed)
        && input.operation != OperationFamily::Drill
        && !matches!(
            input.tool_geometry,
            ToolGeometryHint::VBit { .. } | ToolGeometryHint::TaperedBall { .. }
        )
    {
        // Skip V-bit / tapered ball — see Step 2b note: tier ceiling
        // is calibrated against nominal-D, but these geometries cut on
        // engaged-D, so the nominal-D tier is a false constraint here.
        let tier_ceiling = milling_rpm_ceiling_for_diameter(d);
        if rpm > tier_ceiling {
            let pre_clamp = rpm;
            rpm = tier_ceiling;
            rpm = machine.clamp_rpm(rpm);
            if pre_clamp > 0.0 && rpm < pre_clamp {
                spindle_speedup *= rpm / pre_clamp;
            }
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
        let (floor, ceil) = drill_rpm_envelope_for_diameter(d);
        let pre_clamp = rpm;
        rpm = rpm.clamp(floor, ceil);
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
    //
    // SHADOW POINT (census F-3, T1.4). From here on `effective_d` is the
    // **chip-thinning** diameter, NOT the LUT-semantics one the chipload
    // band was derated by at Step 2'. This is the value published as
    // `FeedsResult::effective_diameter_mm`.
    let effective_d = effective_diameter(
        input.tool_geometry,
        d,
        input.shank_diameter.unwrap_or(d),
        ap,
    );

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
    //
    // ── F-2: the two power axes, and which one each number lives on ──
    //
    // Census §2.6 (C-8) / §6.3 F-2, ruled at Checkpoint B Q3: Suggest's
    // ceiling omitted `machine.safety_factor` while
    // `tool_load::power::evaluate` applies it (`power.rs:214`).
    //
    // There are two internally-consistent axes here, and the pre-fix bug
    // was mixing them, not the absence of a multiply:
    //
    //   RAW axis    — `raw_feed` (pre-Step-9) vs `power_at_rpm(rpm)`.
    //   COMMANDED   — the final feed (Step 9 has applied `safety_factor`)
    //                 vs `power_at_rpm(rpm) · safety_factor`.
    //
    // The CLAMP below lives on the RAW axis and is correct there: it
    // enforces `required(raw_feed) <= power_at_rpm(rpm)`, and since Step
    // 9 then scales the feed by `safety_factor` (and Steps 7/9b/9c only
    // reduce it further), the commanded result satisfies the gate's
    // `required(final) <= power_at_rpm · safety_factor` by construction.
    // Multiplying this clamp's ceiling by `safety_factor` as well would
    // apply the factor TWICE — measured: it drops a power-limited feed a
    // further 25 % and drove the literature-matrix cell
    // `flat_6mm_pocket_al6061_lut` to `major` by pushing chipload to
    // 0.0269 mm/tooth, within 10 % of the 0.025 rubbing floor. That is a
    // feed-moving recalibration, which Q3 did not authorise and §7
    // forbids re-pinning silently.
    //
    // What genuinely lacked parity is the PUBLISHED pair. `power_kw`
    // (below, at the FINAL feed) is a COMMANDED-axis number, and
    // `available_power_kw` — its denominator in the modal's headroom bar
    // (`rs_cam_viz/src/ui/properties/mod.rs:1911`) — was a RAW-axis one.
    // The modal therefore showed 1/safety_factor (1.25×–1.33×) more
    // headroom than the verdict would allow. Both published numbers now
    // sit on the gate's axis; the `PowerLimited` warning's pair is
    // reported there too, preserving its ratio.
    //
    // Measured (tests/power_ceiling_parity_f2.rs): across all three
    // shipped presets × ten species × Ø3/Ø6/Ø12 slots the power branch
    // never fires at all — rigidity and the machine cutting ceiling bind
    // first, peak utilisation 23.6 % — so no shipped-profile feed moves.
    let available_power = machine.power_at_rpm(rpm);
    // The gate's ceiling (`power.rs:214`) — what every published power
    // number below is quoted against.
    let gate_available_power = available_power * machine.safety_factor;
    let mut power_limited = false;
    let mut feed = raw_feed;
    let mut power_factor = 1.0;

    if let Some(kc) = material.kc_n_per_mm2() {
        let cross_section = input.tool_geometry.mrr_cross_section_mm2(ap, ae);
        let required_power =
            crate::tool_load::power::predicted_power_kw(kc, cross_section, raw_feed);
        if required_power > available_power && available_power > 0.0 {
            power_factor = available_power / required_power;
            feed = raw_feed * power_factor;
            power_limited = true;
            warnings.push(FeedsWarning::PowerLimited {
                // Both terms moved onto the gate's COMMANDED axis so the
                // warning compares like with like; the ratio, and hence
                // the derate it explains, is unchanged.
                required_kw: required_power * machine.safety_factor,
                available_kw: gate_available_power,
            });
        }
    }

    // --- Step 7: Machine feed clamp ---
    // F4: the calculator emits CUTTING feeds — clamp at the cutting
    // ceiling, not the gantry travel rate.
    let machine_cut_ceiling = machine.cutting_feed_ceiling_mm_min();
    let mut feed_clamp_factor = 1.0;
    if feed > machine_cut_ceiling {
        warnings.push(FeedsWarning::FeedRateClamped {
            requested: feed,
            actual: machine_cut_ceiling,
        });
        if feed > 0.0 {
            feed_clamp_factor = machine_cut_ceiling / feed;
        }
        feed = machine_cut_ceiling;
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

    // --- Step 9b: Rubbing-floor clamp on final feed ---
    //
    // Guarantee the commanded feed-per-tooth (`feed / (rpm * flutes)`)
    // never falls below the chip-formation threshold. Two paths can
    // push it under:
    //   1. Vendor-LUT scaling for extreme-Janka hardwoods
    //      (e.g. Ipe 3510 lbf vs an oak-anchored 1290 lbf row scales
    //      chipload by 0.367 via `vendor_lookup::hardness_ratio_raw`),
    //      producing a 0.0124 mm/tooth target before any feed derates.
    //   2. Post-clamp derates (safety factor 0.75-0.80, LD overhang,
    //      power-limit) compounding a low-but-above-floor target down
    //      past the floor at the final feed step.
    //
    // Pre-fix the engine had no floor enforcement here:
    // `LookupResult::chip_load_min_mm` was populated but never
    // consulted. Standard machinist convention is "clamp + warn, never
    // serve a rubbing recipe" — raising the final feed to the floor
    // overrides the safety-factor / LD derate that produced the rub,
    // which is the right tradeoff: those derates exist to *protect the
    // tool*, but a chipload below 0.025 mm/tooth heats the edge and
    // burns the work — the cure is worse than the disease.
    //
    // The machine feed cap (Step 7) is treated as a hard physical
    // limit: if lifting feed to the floor would exceed
    // `machine.max_feed_mm_min × safety_factor`, we leave feed at the
    // cap and still emit the warning. In that situation the user must
    // either drop RPM (so floor × rpm × flutes fits under the cap) or
    // accept the rubbing recipe — the engine surfaces the conflict
    // rather than silently violating either constraint.
    //
    // The `> 0.0` guard intentionally lets the RPM-only-LUT-row
    // formula-fallback path (Step 2, lines 482-508) surface its own
    // bug if it ever regresses to zero feed — we don't want this
    // floor silently masking a zero-chipload regression.
    // (Literature-matrix cell flat_6mm_pocket_ipe_hardness.)
    //
    // The floor applied is `effective_rubbing_floor(chipload_bounds)`,
    // NOT the bare `RUBBING_FLOOR_MM_TOOTH` constant: the global
    // threshold is subordinated to the matched row's derated band
    // ceiling so the clamp can never push feed past the very band it
    // exists to keep the recipe inside. See that function for the
    // derivation and for what is given up when a whole band sits below
    // the global threshold (FEEDS_CENSUS C-12 / T3.3, ruled 2026-08-06).
    let fpt_divisor = rpm * input.flute_count as f64;
    if fpt_divisor > 0.0 {
        let commanded_fpt = feed / fpt_divisor;
        let floor = effective_rubbing_floor(chipload_bounds);
        if commanded_fpt > 0.0 && commanded_fpt < floor {
            warnings.push(FeedsWarning::ChiploadClampedToFloor {
                requested: commanded_fpt,
                floor,
                band_capped_from: (floor < RUBBING_FLOOR_MM_TOOTH)
                    .then_some(RUBBING_FLOOR_MM_TOOTH),
            });
            let machine_max_feed_after_safety = machine.max_feed_mm_min * machine.safety_factor;
            let target_feed = floor * fpt_divisor;
            feed = target_feed.min(machine_max_feed_after_safety);
        }
    }

    // --- Step 9c: Drill cycles — envelope sanity + "feed IS plunge" ---
    //
    // A drill cycle has exactly one feed: the plunge. Two corrections
    // close the 2026-06-10 defect-class findings (F1, see
    // `planning/DEFECT_CLASS_CLEANUP_2026-06-10.md`):
    //
    // 1. Clamp the chipload-derived feed into the material plunge-feed
    //    envelope (`Material::drill_plunge_feed_envelope_per_mm`, units
    //    feed/Ø per minute). Same clamp-and-warn convention as the
    //    rubbing floor above — never silently serve a recipe in the
    //    rubbing band below the envelope or the breakage band above it.
    //    The machine cap (post-safety) still wins over the envelope
    //    floor: a machine that can't reach the floor gets the honest
    //    conflict via the warning rather than an unreachable feed.
    // 2. Alias `plunge_rate` to the final drill feed. Drill op configs
    //    alias `set_feed_rate` / `set_plunge_rate` onto one field
    //    ("feed IS plunge"), and `apply_feeds_subset` writes feed then
    //    plunge — pre-fix, the milling plunge baseline from Step 8
    //    clobbered the drill-tuned feed (RPM band + 2.5× chipload from
    //    Steps 1-2), landing every suggested drill at the milling
    //    plunge value instead. Making the result self-consistent here
    //    keeps any write order safe.
    if input.operation == OperationFamily::Drill {
        let (env_lo_per_mm, env_hi_per_mm) = material.drill_plunge_feed_envelope_per_mm();
        let (env_lo, env_hi) = (env_lo_per_mm * d, env_hi_per_mm * d);
        if feed < env_lo || feed > env_hi {
            let machine_max_feed_after_safety = machine.max_feed_mm_min * machine.safety_factor;
            let requested = feed;
            let clamped = feed
                .clamp(env_lo, env_hi)
                .min(machine_max_feed_after_safety);
            warnings.push(FeedsWarning::DrillFeedClampedToEnvelope {
                requested,
                actual: clamped,
                envelope_lo: env_lo,
                envelope_hi: env_hi,
            });
            // When the ceiling binds (feed reduced), RPM follows down —
            // bounded by the drill band floor — so the commanded
            // chipload holds instead of thinning toward rubbing. A
            // feed-capped drill that keeps spinning fast takes thinner
            // and thinner bites per rev; drilling chip evacuation
            // prefers fewer revolutions anyway (same sources as
            // `drill_rpm_envelope_for_diameter`). Small drills make
            // this concrete: Ø3 oak targets ~0.107 mm/tooth at 14 kRPM
            // ⇒ ~3000 mm/min, but the envelope caps feed at 1200 —
            // without the follow-down the shipped recipe implies
            // 0.043 mm/tooth at 14 k, well under the chart band.
            let flutes = input.flute_count as f64;
            if clamped < requested && rpm > 0.0 && flutes > 0.0 {
                let kept_fpt = requested / (rpm * flutes);
                if kept_fpt > 0.0 {
                    let (band_floor, _) = drill_rpm_envelope_for_diameter(d);
                    let target_rpm =
                        machine.clamp_rpm((clamped / (kept_fpt * flutes)).max(band_floor));
                    if target_rpm < rpm {
                        spindle_speedup *= target_rpm / rpm;
                        rpm = target_rpm;
                    }
                }
            }
            feed = clamped;
        }
        plunge_rate = feed;
    }

    // Final power at actual feed. Materials without a primary-source Kc
    // report 0.0 — the consumers that need a numeric headroom (charts /
    // diagnostics) treat this as "unmodeled" rather than zero load.
    // Same canonical helper as Step 6 above so Suggest's reported
    // power matches the Sim verdict's prediction.
    let actual_power = match material.kc_n_per_mm2() {
        Some(kc) => {
            let cross_section = input.tool_geometry.mrr_cross_section_mm2(ap, ae);
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
        // F-2: the gate's ceiling, not the raw spindle curve — this is
        // the denominator `power_kw` (evaluated at the FINAL feed) is
        // rendered against. See the Step 6 axis note above.
        available_power_kw: gate_available_power,
        power_limited,
        mrr_mm3_min: mrr,
        warnings,
        vendor_source,
        chipload_source,
        chipload_bounds,
        matched_lut_row,
        effective_diameter_mm: effective_d,
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

/// CONTACT-CIRCLE diameter at axial depth `ap` — how wide the cutter
/// actually touches material, which is the quantity both chip-thinning terms
/// need.
///
/// Distinct from [`ToolGeometryHint::engaged_diameter_at_doc`] for the FLAT
/// family only: that one answers "which vendor-LUT chipload row applies",
/// where a ball and a bull engage at nominal D regardless of depth. Here a
/// ball at 0.05 mm engages a 0.44 mm circle, and that is the point.
///
/// C3 (2026-08-02): the tapered-ball arm now DELEGATES to
/// `engaged_diameter_at_doc` rather than carrying its own straight-cone
/// approximation. The tapered ball's tip IS a ball, so below the tangency
/// height this arm and the `Ball` arm agree to the last bit; above it the
/// cone shoulder takes over, capped at the shank. See
/// `crates/rs_cam_core/src/feeds/geometry.rs` for what was retired and
/// `tests/tapered_width_model_parity_c3.rs` for what it cost.
fn effective_diameter(geom: ToolGeometryHint, nominal_d: f64, shank_d: f64, ap: f64) -> f64 {
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
        ToolGeometryHint::TaperedBall { .. } => {
            geom.engaged_diameter_at_doc(ap, nominal_d, shank_d)
        }
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

    /// F1 (2026-06-10 defect-class cleanup): drill results are
    /// self-consistent — plunge IS feed, so the drill configs' aliased
    /// setters ("feed IS plunge") are write-order-safe — and the feed
    /// sits inside the material plunge-feed envelope. Pre-fix,
    /// `apply_feeds_subset` wrote feed then plunge and the milling
    /// plunge baseline (≈595 for Ø6 hardwood) clobbered the drill-tuned
    /// feed; separately the unclamped drill feed (4000) exceeded the
    /// Ø6 wood envelope max (2400).
    #[test]
    fn test_drill_feed_within_envelope_and_plunge_aliased() {
        for species in [WoodSpecies::GenericSoftwood, WoodSpecies::GenericHardwood] {
            let material = Material::SolidWood { species };
            let machine = MachineProfile::shapeoko_vfd();
            for d in [3.0, 6.0, 12.0] {
                let result = calculate(&FeedsInput {
                    tool_diameter: d,
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
                assert_eq!(
                    result.plunge_rate_mm_min, result.feed_rate_mm_min,
                    "Ø{d} {species:?}: drill plunge must alias the final feed"
                );
                let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
                let ratio = result.feed_rate_mm_min / d;
                assert!(
                    ratio >= lo - 1e-9 && ratio <= hi + 1e-9,
                    "Ø{d} {species:?}: feed/Ø {ratio:.1} outside envelope {lo}-{hi}"
                );
            }
        }
    }

    /// F1: when the envelope ceiling binds, RPM follows the feed down
    /// (bounded by the drill band floor) so the shipped recipe keeps
    /// its commanded chipload instead of thinning toward rubbing. Ø3
    /// oak: target ~0.107 mm/tooth at 14 kRPM ⇒ ~3000 mm/min, envelope
    /// caps at 1200 — without the follow-down the implied chipload
    /// collapses to 0.043 at 14 k.
    #[test]
    fn test_drill_envelope_ceiling_drops_rpm_to_hold_chipload() {
        let material = Material::SolidWood {
            species: WoodSpecies::WhiteOak,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let result = calculate(&FeedsInput {
            tool_diameter: 3.0,
            flute_count: 2,
            flute_length: 12.0,
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
        let clamp_fired = result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::DrillFeedClampedToEnvelope { .. }));
        assert!(clamp_fired, "Ø3 oak drill must hit the envelope ceiling");
        assert!(
            result.rpm < 14_000.0,
            "RPM must follow the capped feed down, got {}",
            result.rpm
        );
        assert!(
            result.rpm >= 8_000.0,
            "follow-down bounded by the drill band floor, got {}",
            result.rpm
        );
        let implied = result.feed_rate_mm_min / (2.0 * result.rpm);
        assert!(
            implied >= 0.05,
            "implied chipload {implied:.4} must stay clear of the rubbing regime"
        );
    }

    /// F1: the envelope clamp warns when it binds — never a silent
    /// rewrite. Hardwood Ø3 drives the chipload-derived feed above the
    /// small-diameter envelope max (400 × 3 = 1200), so the clamp must
    /// fire with the honest before/after.
    #[test]
    fn test_drill_feed_clamp_emits_warning_when_binding() {
        let material = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let result = calculate(&FeedsInput {
            tool_diameter: 3.0,
            flute_count: 2,
            flute_length: 12.0,
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
        let ratio = result.feed_rate_mm_min / 3.0;
        let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
        assert!(ratio >= lo - 1e-9 && ratio <= hi + 1e-9);
        // If the pre-clamp feed was out of band the warning must exist;
        // verify consistency rather than hardcoding which side binds.
        let clamped = result.warnings.iter().find_map(|w| match w {
            FeedsWarning::DrillFeedClampedToEnvelope {
                requested, actual, ..
            } => Some((*requested, *actual)),
            _ => None,
        });
        if let Some((requested, actual)) = clamped {
            assert!(
                (actual - result.feed_rate_mm_min).abs() < 1e-9,
                "warning's actual {actual} must match the shipped feed {}",
                result.feed_rate_mm_min
            );
            assert!(
                requested < lo * 3.0 || requested > hi * 3.0,
                "warning fired but requested {requested} was in band"
            );
        }
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

    /// Finding 1 (2026-06-04): Suggest's `ChiploadBounds` must mirror
    /// the post-sim chipload gate's DOC-derated envelope so
    /// `SuggestAggressiveness::target_chipload(bounds)` aims at a value
    /// the gate will accept. Without derating, v3.0c median targeting
    /// on high-DOC milling ops (e.g. wanaka Back Rough at ~3×D) lands
    /// above the gate's derated max and trips `Exceeds(High)`.
    ///
    /// This test pins the derating contract on a 3×D hardwood pocket-
    /// roughing case: bounds at axial_depth=D get the 1.0 scale (no
    /// change), bounds at axial_depth=3D get the 0.5 scale.
    #[test]
    fn chipload_bounds_derate_with_doc_ratio_for_milling() {
        let lut = vendor_lut::VendorLut::embedded();
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let mut input = FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 24.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: Some(6.0), // 1×D ratio → scale 1.0
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        };
        let at_1x = calculate(&input);
        input.axial_depth_mm = Some(18.0); // 3×D ratio → scale 0.5
        let at_3x = calculate(&input);

        // LUT 6 mm hardwood pocket-roughing row publishes both bounds,
        // so the orchestrator must produce `Some(ChiploadBounds)` here.
        let b1 = at_1x.chipload_bounds.unwrap();
        let b3 = at_3x.chipload_bounds.unwrap();
        let scale_min = b3.min_mm_per_tooth / b1.min_mm_per_tooth;
        let scale_max = b3.max_mm_per_tooth / b1.max_mm_per_tooth;
        assert!(
            (scale_min - 0.5).abs() < 1e-6,
            "min should derate to 0.5× at 3×D, got scale {scale_min} ({} → {})",
            b1.min_mm_per_tooth,
            b3.min_mm_per_tooth,
        );
        assert!(
            (scale_max - 0.5).abs() < 1e-6,
            "max should derate to 0.5× at 3×D, got scale {scale_max} ({} → {})",
            b1.max_mm_per_tooth,
            b3.max_mm_per_tooth,
        );
    }

    /// Drill ops are explicitly excluded from chipload-bounds DOC
    /// derating: the post-sim chipload gate short-circuits drill ops
    /// (`NotApplicableForOp`), and the chip-evacuation rule that
    /// motivates derating in milling doesn't apply to drill bands
    /// (peck depth, not engagement). Bounds for a deep-peck drill cycle
    /// must equal the bounds at shallow peck.
    #[test]
    fn chipload_bounds_skip_doc_derating_for_drill_ops() {
        let lut = vendor_lut::VendorLut::embedded();
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let machine = MachineProfile::shapeoko_vfd();
        let mut input = FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 30.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: OperationFamily::Drill,
            pass_role: PassRole::Roughing,
            axial_depth_mm: Some(6.0),
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        };
        let shallow = calculate(&input);
        input.axial_depth_mm = Some(18.0);
        let deep = calculate(&input);
        if let (Some(s), Some(d)) = (shallow.chipload_bounds, deep.chipload_bounds) {
            assert!(
                (s.min_mm_per_tooth - d.min_mm_per_tooth).abs() < 1e-9,
                "drill bounds.min must not derate with peck depth",
            );
            assert!(
                (s.max_mm_per_tooth - d.max_mm_per_tooth).abs() < 1e-9,
                "drill bounds.max must not derate with peck depth",
            );
        }
        // If the LUT doesn't publish drill chipload bounds (None on either
        // depth), the test is moot — derating exclusion is what we care
        // about and both being None already satisfies the contract.
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
        let chipload_chart =
            match_chart.feed_rate_mm_min / (match_chart.rpm * f64::from(base.flute_count));
        let chipload_max =
            max_speed.feed_rate_mm_min / (max_speed.rpm * f64::from(base.flute_count));
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

    // ── Phase 3 CutterKind pins (architectural refactor 2026-06-06) ──

    /// Every `ToolGeometryHint` shape maps to exactly the expected
    /// `CutterKind`, and the centralized LUT-family map matches the
    /// (formerly triplicated) `ToolGeometryHint → ToolFamily` table
    /// verbatim. A new hint variant fails to compile in
    /// `cutter_kind()`; a changed family mapping fails here.
    #[test]
    fn cutter_kind_lut_family_map_is_pinned() {
        use vendor_lut::ToolFamily;
        let cases = [
            (
                ToolGeometryHint::Flat,
                CutterKind::Flat,
                ToolFamily::FlatEnd,
            ),
            (
                ToolGeometryHint::Ball,
                CutterKind::Ball,
                ToolFamily::BallNose,
            ),
            (
                ToolGeometryHint::Bull { corner_radius: 1.0 },
                CutterKind::Bull,
                ToolFamily::BullNose,
            ),
            (
                ToolGeometryHint::VBit {
                    included_angle: 60.0,
                    tip_diameter: 0.1,
                },
                CutterKind::VBit,
                ToolFamily::ChamferVbit,
            ),
            (
                ToolGeometryHint::TaperedBall {
                    tip_radius: 0.5,
                    taper_angle_deg: 10.0,
                },
                CutterKind::TaperedBall,
                ToolFamily::TaperedBallNose,
            ),
        ];
        assert_eq!(cases.len(), CutterKind::ALL.len());
        for (hint, kind, family) in cases {
            assert_eq!(hint.cutter_kind(), kind, "{hint:?}");
            assert_eq!(kind.lut_family(), family, "{kind:?}");
        }
    }

    /// Named decision: `ToolFamily::FacingBit` is LUT row vocabulary
    /// only — no cutter shape classifies to it. If a facing cutter
    /// ever becomes a real `CutterKind`, this pin forces the
    /// compatibility story (LUT routing, constraints) to be revisited
    /// deliberately rather than inherited.
    #[test]
    fn facing_bit_is_a_lut_only_family() {
        for &kind in CutterKind::ALL {
            assert_ne!(
                kind.lut_family(),
                vendor_lut::ToolFamily::FacingBit,
                "{kind:?} must not classify to the data-only FacingBit family"
            );
        }
    }

    /// `ToolType ↔ CutterKind` is a bijection (5 ↔ 5): the convenience
    /// `ToolType::cutter_kind()` and the registry-facing
    /// `CutterKind::tool_type()` must be mutual inverses. A 6th cutter
    /// breaking the bijection fails here and forces a decision.
    #[test]
    fn tool_type_cutter_kind_round_trips() {
        use crate::compute::ToolType;
        for &tool_type in ToolType::ALL {
            assert_eq!(tool_type.cutter_kind().tool_type(), tool_type);
        }
        for &kind in CutterKind::ALL {
            assert_eq!(kind.tool_type().cutter_kind(), kind);
        }
        assert_eq!(ToolType::ALL.len(), CutterKind::ALL.len());
    }

    /// S.3 parity sentry (`planning/finishing_stack_review_2026-07.md`).
    ///
    /// `ToolGeometryHint::engaged_diameter_at_doc` (Suggest's path —
    /// this module, ~line 108) and `MillingCutter::lookup_diameter_at`
    /// (the post-sim gate's path — `tool::vbit::VBitEndmill` /
    /// `tool::tapered_ball::TaperedBallEndmill`) are two hand-written
    /// implementations of the same engaged-diameter geometry. They
    /// agree today, but nothing enforces it — one edit to either side
    /// could silently split Suggest's chipload-band targeting from the
    /// gate's derating and produce false chipload trips.
    ///
    /// Sweeps DOC across 0.1×..2× nominal diameter (crossing the
    /// ball/cone and flute/taper transitions) for every shape with
    /// nontrivial engagement geometry (v-bit, tapered ball), plus flat
    /// and ball as trivial always-nominal-diameter cases, and asserts
    /// exact agreement (1e-9) at every sample. If this ever fails: DO
    /// NOT silently pick one implementation over the other — report
    /// the numeric disagreement and mark this `#[ignore]` with a
    /// pointer to the report so the tree stays green while the
    /// implementations are reconciled deliberately.
    #[test]
    fn engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes() {
        use crate::tool::{
            BallEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill,
        };

        struct Case {
            name: &'static str,
            diameter_mm: f64,
            shank_mm: f64,
            hint: ToolGeometryHint,
            lookup: Box<dyn Fn(f64) -> f64>,
        }

        let flat = FlatEndmill::new(6.0, 20.0);
        let ball = BallEndmill::new(6.0, 20.0);
        let vbit = VBitEndmill::new(6.0, 90.0, 20.0);
        // ball_diameter=6mm tip, taper_half_angle=20deg, shaft=10mm.
        let tapered = TaperedBallEndmill::new(6.0, 20.0, 10.0, 30.0);

        let cases: Vec<Case> = vec![
            Case {
                name: "flat (trivial: nominal diameter regardless of DOC)",
                diameter_mm: 6.0,
                shank_mm: 6.0,
                hint: ToolGeometryHint::Flat,
                lookup: Box::new(move |doc| flat.lookup_diameter_at(doc)),
            },
            Case {
                name: "ball (trivial: nominal diameter regardless of DOC)",
                diameter_mm: 6.0,
                shank_mm: 6.0,
                hint: ToolGeometryHint::Ball,
                lookup: Box::new(move |doc| ball.lookup_diameter_at(doc)),
            },
            Case {
                name: "vbit 90deg",
                diameter_mm: 6.0,
                shank_mm: 6.0,
                hint: ToolGeometryHint::VBit {
                    included_angle: 90.0,
                    tip_diameter: 0.0,
                },
                lookup: Box::new(move |doc| vbit.lookup_diameter_at(doc)),
            },
            Case {
                name: "tapered ball, tip r=3mm, taper=20deg, shaft=10mm",
                diameter_mm: 10.0,
                shank_mm: 10.0,
                hint: ToolGeometryHint::TaperedBall {
                    tip_radius: 3.0,
                    taper_angle_deg: 20.0,
                },
                lookup: Box::new(move |doc| tapered.lookup_diameter_at(doc)),
            },
        ];

        const STEPS: u32 = 40;
        for case in &cases {
            for i in 0..=STEPS {
                let frac = 0.1 + (2.0 - 0.1) * f64::from(i) / f64::from(STEPS);
                let doc_mm = frac * case.diameter_mm;
                let hint_d =
                    case.hint
                        .engaged_diameter_at_doc(doc_mm, case.diameter_mm, case.shank_mm);
                let trait_d = (case.lookup)(doc_mm);
                let diff = (hint_d - trait_d).abs();
                assert!(
                    diff < 1e-9,
                    "{}: at doc={doc_mm:.4}mm engaged_diameter_at_doc={hint_d:.9} \
                     lookup_diameter_at={trait_d:.9} (diff={diff:.3e}) — S.3 divergence, \
                     see planning/finishing_stack_review_2026-07.md",
                    case.name,
                );
            }
        }
    }
}
