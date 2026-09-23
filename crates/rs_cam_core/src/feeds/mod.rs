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
pub mod efficiency;
pub mod explain_payload;
pub mod feed_explanation;
pub mod force;
pub mod geometry;
pub mod geometry_class;
pub mod operating_point;
pub mod predict;
pub mod profile;
pub mod provenance;
pub mod quantities;
pub mod rationale;
pub mod suggest;
pub mod support;
pub mod vendor_lookup;
pub mod vendor_lut;
pub mod vendor_normalize;
pub use efficiency::{ChipVerdict, CutEfficiency, cut_efficiency};
pub use explain_payload::{FeedsExplain, MachineEnvelope, explain as explain_feeds};
pub use feed_explanation::{
    ADVANCE_PER_TOOTH, AchievedFeedStage, ClampReason, CommandedStage, FeedExplanation,
    GateObservationStage, LutBandStage, ObservedStatistic,
};
pub use operating_point::{PowerFigure, PowerUnmodeled, power_at_operating_point};
pub use predict::{
    DeflectionBreakdown, DeflectionCaveat, DeflectionPrediction, DeflectionUnmodeled,
    predict_peak_deflection_um,
};
pub use provenance::{FeedsField, FeedsProvenance, ProvenanceSource, ValueProvenance};
pub use quantities::{
    ACHIEVED_ADVANCE_PER_TOOTH, ADVANCE_PER_TOOTH_UNIT, ARC_MEAN_CHIP_THICKNESS, AchievedFeedMmMin,
    AdvancePerToothMm, ArcMeanChipThicknessMm, COMMANDED_ADVANCE_PER_TOOTH, ChiploadBandClass,
    CommandedFeedMmMin, VendorChiploadBand,
};
pub use support::{FeedsSupport, feeds_support};
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
    /// (`tool_load::power::evaluate`) agree on what area the power
    /// model's SHEAR term multiplies by. R1's edge term reads no
    /// cross-section at all — see `tool_load::power::PowerTerms`.
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
    /// The kind as words, for operator-facing text.
    pub fn label(self) -> &'static str {
        match self {
            CutterKind::Flat => "flat end mill",
            CutterKind::Ball => "ball nose",
            CutterKind::Bull => "bull nose",
            CutterKind::VBit => "V-bit",
            CutterKind::TaperedBall => "tapered ball nose",
        }
    }

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
    /// **The operation's own kind.** Checkpoint K (a4), 2026-08-13.
    ///
    /// [`OperationFamily`] above is the *vendor-LUT* family the operation
    /// declares (eight values). It cannot distinguish `Adaptive3d` from
    /// 2D adaptive, nor `ProjectCurve` from `Trace` — and those are
    /// exactly the two kinds
    /// [`vendor_normalize::lut_query_for`] reroutes. Before a4 the
    /// Suggest side had no way to ask, so it queried the declared family
    /// while the gate queried the routed one: 489 of 3 024 pairs resolved
    /// to different rows and 378 more left Suggest banded where the gate
    /// refused (A-6 census, `LUT_BOUNDARY_EVIDENCE.md` §2).
    ///
    /// `None` means "no operation identity available", and the routing is
    /// then a **no-op** — the declared family is used unchanged, i.e. the
    /// pre-a4 behaviour. Every production Suggest path supplies it via
    /// `suggest::feeds_input_for_operation`; the `None` arm exists for
    /// unit fixtures that construct a `FeedsInput` with no operation
    /// behind it.
    pub operation_kind: Option<crate::compute::catalog::OperationType>,
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
    /// Predicted spindle power (kW) **at the CALCULATOR's geometry** — the
    /// `ap`, `ae`, RPM and feed this result carries, on the COMMANDED axis.
    ///
    /// It is not the power the machine draws. `suggest::enforce_invariants`
    /// runs after the calculator and may lower the depth per pass (the
    /// rigidity, cutting-length and deflection clamps) and the stepover, and
    /// pass 9 then re-derives the feed at the point that ships. A display of
    /// the SHIPPED figure calls
    /// [`crate::feeds::power_at_operating_point`] on the final operation.
    pub power_kw: f64,
    /// The ceiling `power_kw` is quoted against (kW):
    /// `power_at_rpm(rpm) × safety_factor`, the gate's own ceiling.
    ///
    /// It moves with the RPM, so it belongs to the calculator's operating
    /// point exactly as `power_kw` does. See `power_kw` for the shipped
    /// figure.
    pub available_power_kw: f64,
    pub power_limited: bool,
    pub mrr_mm3_min: f64,
    pub warnings: Vec<FeedsWarning>,
    /// Observation ID if vendor LUT was used for chipload.
    pub vendor_source: Option<String>,
    /// The support arm of this cell ([`FeedsSupport`]). `calculate` sets it
    /// from the same row lookup that sets `matched_lut_row`, so a consumer
    /// reads the arm here and does not derive it again.
    pub support: FeedsSupport,
    pub chipload_source: ChiploadSource,
    /// LUT-derived chipload band for the matched vendor row, when the
    /// match supplied one. `None` for formula-fallback / RPM-only LUT
    /// rows / edge-radius-floor paths.
    ///
    /// The band is de-rated at the SHIPPED axial depth
    /// ([`Self::axial_depth_mm`]) by `geometry::doc_derating_scale`, the
    /// same scale the post-sim chipload gate applies (feeds matrix R3).
    ///
    /// Until 2026-08-13 this was also the input to Suggest's chipload-aware
    /// feed-up recalibration (pass 8), which is retired — see the retirement
    /// note in [`crate::feeds::predict`]. The band still drives the
    /// rationale, the chipload envelopes and the post-sim gate; it no longer
    /// moves a feed before simulation.
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
/// `target_chip_load_mm × combined_factor()` — note the accessor, **not**
/// the product of every field. Since 2026-08-19 three fields here
/// (`observed_*_chip_thinning`) are measurements the engine reports but does
/// not apply, so multiplying the struct out by hand overstates the feed by up
/// to 4×. `combined_factor()` is the only correct composition.
///
/// Derived purely so the UI can render the breakdown — calculate()
/// applies each APPLIED factor in place, this struct just captures them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeedsDerates {
    /// LUT midpoint (or formula chipload) before any multipliers.
    pub target_chip_load_mm: f64,
    /// Empirical formula breakdown — populated only when the chipload
    /// came from [`ChiploadSource::FormulaFallback`] / `EdgeRadiusFloor`.
    /// `None` when the LUT supplied the value.
    pub formula: Option<FormulaBreakdown>,
    /// Radial chip-thinning factor (≥ 1.0) — **OBSERVED, NOT APPLIED** since
    /// 2026-08-19 (G-CHIPTHIN-HALFFIX).
    ///
    /// The geometric statement is real: at a small stepover the chip is
    /// thinner per tooth-pass. What is gone is the inference "…so feed
    /// faster", because the vendor column this would correct publishes no
    /// radial reference condition to correct *from* — see the deletion note at
    /// Step 5 of [`calculate`] and `CHIPLOAD_LITERATURE_VERDICT.md` N-8.
    ///
    /// Reported so the condition stays visible to the operator and to
    /// diagnostics. It is deliberately **excluded** from
    /// [`FeedsDerates::combined_factor`]: a number in this struct that does not
    /// multiply the feed must not be composed with the ones that do.
    pub observed_radial_chip_thinning: f64,
    /// Axial chip-thinning factor for ball / tapered-ball tools at shallow DOC
    /// — **OBSERVED, NOT APPLIED**, same ruling and same reasoning as
    /// [`Self::observed_radial_chip_thinning`]. The vendor charts publish a
    /// rule for cutting deeper than 1 × D and none at all for shallower.
    pub observed_axial_chip_thinning: f64,
    /// Combined observed chip thinning, clamped to `[1.0, 4.0]` — **OBSERVED,
    /// NOT APPLIED**. The clamp is retained because the reported number should
    /// stay the one the engine historically computed, so the survey that
    /// justified the deletion remains comparable against it.
    pub observed_combined_chip_thinning: f64,
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
    /// Multiplier applied to the RPM while holding the chipload — a walk
    /// along the constant-chipload line, in either direction. The feed
    /// scales with it, so the advance per tooth does not move.
    ///
    /// **Was `spindle_scale`, renamed 2026-09-16.** It could only ever
    /// exceed 1.0, because the only thing that moved it was
    /// [`SpindleStrategy::MaxSpeed`] lifting RPM toward the spindle
    /// ceiling. The power ladder walks the same line DOWNWARD on a
    /// constant-power spindle, so the value can now be below 1.0 and a
    /// field called "speedup" holding 0.5 would be a lie.
    ///
    /// Read [`Self::spindle_scale_reason`] before rendering this. A bare
    /// factor cannot tell an operator whether the spindle sped up by
    /// policy or slowed down because the cut was over power.
    ///
    /// NOT part of [`combined_factor`], deliberately: this walks the
    /// constant-chipload line, so the chipload is unchanged.
    ///
    /// [`combined_factor`]: Self::combined_factor
    pub spindle_scale: f64,
    /// Why [`Self::spindle_scale`] is not 1.0.
    pub spindle_scale_reason: SpindleScaleReason,
}

/// What moved the spindle off the RPM the chart or formula chose.
///
/// Added 2026-09-16 with the power ladder. Before it, `why.rs` rendered
/// "Spindle policy MaxSpeed lifted it" for ANY scale that was not 1.0,
/// because `MaxSpeed` was the only thing that could move it. Once a power
/// limit can move it the other way, that sentence names the wrong cause —
/// the same defect shape this programme has been clearing throughout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpindleScaleReason {
    /// The scale is 1.0. The RPM is the chart's or the formula's.
    #[default]
    Unchanged,
    /// [`SpindleStrategy::MaxSpeed`] lifted the RPM toward the spindle
    /// ceiling and scaled the feed with it.
    MaxSpeedPolicy,
    /// The cut was over the spindle's power budget and the spindle is one
    /// where a slower RPM genuinely reduces the load. See the power ladder
    /// in [`calculate`].
    PowerLimit,
}

/// Empirical chipload formula evaluation `K₀ × D^p × (1/H)^q`.
/// Captured so the UI can show *why* the no-LUT recommendation is what
/// it is (rather than just "fallback").
///
/// Stays `pub`: it is the payload of the `pub` field `FeedsDerates::formula`,
/// so a crate-private form raises `private_interfaces`.
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
    /// Compose every **applied** multiplier into a single number. The effective
    /// chipload (`feed / (RPM × flutes)`) equals
    /// `target_chip_load_mm × combined_factor()`.
    pub fn combined_factor(&self) -> f64 {
        // Spindle speedup is intentionally NOT included here:
        // `combined_factor` represents the multiplier applied to the
        // target chipload to get the effective chipload. Spindle
        // speedup walks the constant-chipload line (RPM and feed
        // scale together), so chipload is unchanged. The modal
        // renders `spindle_scale` as a peer row so the operator
        // sees the speed-axis change separately from the chipload
        // derates.
        //
        // The three `observed_*_chip_thinning` fields are NOT included either,
        // and for a different reason: since 2026-08-19 they are measurements
        // rather than derates, and `calculate` no longer multiplies the feed by
        // them. Composing them here would make this function — and the
        // `effective_chip_load_mm` identity built on it — disagree with the
        // feed the engine actually emits. See their field docs.
        self.depth_tier
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
    /// The power ladder made the cut smaller to fit the spindle's budget.
    ///
    /// The operator asked for one cut and the engine is recommending a
    /// different one, so this always surfaces. It is NOT an error: the cut
    /// fits now, or [`Self::PowerLimited`] fires alongside it saying it still
    /// does not.
    ///
    /// The advance per tooth is unchanged in every case. An RPM move walks
    /// the constant-chipload line, and a depth or width move touches neither
    /// the feed nor the RPM. The cut got smaller, not thinner — which is the
    /// whole point, because a thinner chip is what the pre-ladder feed derate
    /// produced and it did not fix the constraint.
    ///
    /// Each pair is `Some` only for a dial the ladder actually moved.
    PowerLadderReducedCut {
        rpm_from: Option<f64>,
        rpm_to: Option<f64>,
        axial_from: Option<f64>,
        axial_to: Option<f64>,
        radial_from: Option<f64>,
        radial_to: Option<f64>,
        /// The LAST-RESORT feed multiplier, when the rungs above could not
        /// close the gap on their own. `Some` here is the one case where the
        /// advance per tooth DID change — every other dial holds it.
        feed_factor: Option<f64>,
        /// Draw before the ladder ran, on the commanded axis.
        required_kw_before: f64,
        /// Draw after it ran. Above `available_kw` when the ladder ran out
        /// of rungs, in which case `PowerLimited` fires too.
        required_kw_after: f64,
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
    /// **Checkpoint K (a3), 2026-08-13 — the recipe resolver matched a
    /// row that publishes no chipload column, so the recommendation is a
    /// formula number and there is no band at all.**
    ///
    /// The two LUT entry points have declared, different purposes:
    /// [`vendor_lookup::find_best_row_for_geometry`] (the recipe
    /// resolver) lets RPM-only anchors compete, while
    /// [`vendor_lookup::find_best_chip_envelope_row`] (the envelope
    /// resolver the gate uses) excludes them. A-6's census measured the
    /// consequence at **141 of 18 144** swept queries — three cells,
    /// all `Adaptive`/Flat, `Adaptive`/Bull and `Trace`/VBit60.
    ///
    /// In those cases the operator sees a vendor row id beside a
    /// chipload the vendor never published, `chipload_bounds` is `None`
    /// (so `SuggestAggressiveness::target_chipload` has nothing to aim
    /// at), and the post-sim gate judges against a *different* row's
    /// band. Nothing said so before this warning.
    ///
    /// **P1, 2026-08-22 — one of those consequences is now fixed.**
    /// `effective_rubbing_floor` used to fall back to the bare
    /// [`RUBBING_FLOOR_MM_TOOTH`] here; it now takes the envelope
    /// resolver's band, so the floor and the gate quote the same row and
    /// `floor_band_from` names it. What is deliberately NOT fixed is the
    /// target: the recipe still rests on the RPM anchor and still carries
    /// no band, because that is what having two resolvers is *for*.
    ///
    /// Otherwise report-only: the RPM anchor is still used — that is the
    /// point of keeping two resolvers (option a3, not a1).
    VendorRowPublishesNoChipload {
        /// The RPM-anchor row the recipe resolver matched.
        observation_id: String,
        /// The empirical-formula advance per tooth used in place of the
        /// absent vendor column (mm/tooth).
        formula_chipload_mm: f64,
        /// P1 (2026-08-22) — the row the **rubbing floor** was subordinated
        /// to instead, from the envelope resolver, or `None` when that
        /// resolver found nothing either and the bare
        /// [`RUBBING_FLOOR_MM_TOOTH`] still applies.
        ///
        /// This is the one thing the recommendation now *does* take from a
        /// chipload-bearing row. The target still comes from the formula and
        /// `chipload_bounds` is still `None`, so this field is not a band —
        /// it is provenance for a single clamp, and naming it is what keeps
        /// "the floor" and "the gate's envelope" from being two different
        /// rows the operator cannot tell apart.
        floor_band_from: Option<String>,
    },
    /// **Checkpoint K (a4), 2026-08-13 — the vendor-LUT routing refused
    /// this operation × cutter pairing, so there is no vendor row at all
    /// and the recommendation is entirely formula-derived.**
    ///
    /// Today this is exactly one case: a `ProjectCurve` on a bull-nose,
    /// V-bit or facing cutter. `ProjectCurve` is not a vendor family; it
    /// is geometrically a 3D contour trace, and
    /// [`vendor_normalize::lut_query_for`] routes it to `Parallel` /
    /// `Contour` for ball and flat cutters and refuses for the rest,
    /// because the LUT has no rows there.
    ///
    /// The gate has refused these all along
    /// (`Unmodeled(NoVendorData)`). Until a4 **Suggest did not** — it
    /// queried the unrouted `Trace` family and returned a confident
    /// vendor-backed band on a surface where the gate declined to judge,
    /// with nothing on any screen saying the two were talking about
    /// different rows. A-6 counted **378** such pairs. Checkpoint K
    /// ruled the refusal in: more honest and less useful, and the
    /// alternative needs LUT rows that do not exist.
    NoVendorRowsForRoutedOperation {
        /// The operation kind the routing refused, as a debug string.
        operation_kind: String,
        /// The cutter class it refused for.
        tool_family: String,
        /// Which rows are missing — the refusal names them rather than
        /// saying "no data".
        missing_rows: String,
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
    /// The tool cannot run the operation. Two rules raise it, both the
    /// engine's own (feeds matrix ruling R1, 2026-09-23):
    ///
    /// - the operation's registry row does not allow the tool's cutter
    ///   kind (`OpRegistryEntry::tool_constraints`, the same predicate the
    ///   generator refuses on) — `allowed` lists the kinds the row allows;
    /// - a scallop-height stepover needs a curved tip: `Scallop`, and
    ///   `Parallel` with a scallop target, refuse `Flat` and `VBit`, whose
    ///   tip radius is zero and whose scallop-stepover formula
    ///   `2·√(2·R·h − h²)` is undefined — `allowed` is empty.
    WrongToolForOperation {
        /// The operation, when the caller named one; a raw `calculate`
        /// caller may know only the feeds family.
        operation: Option<crate::compute::catalog::OperationType>,
        family: OperationFamily,
        actual: CutterKind,
        /// The kinds the registry row allows; empty for the scallop-tip rule.
        allowed: &'static [CutterKind],
    },
    /// The engine has no basis for this cell: no vendor row matches, and
    /// either the operation declares no formula source
    /// (`OperationSpec::feeds_formula_source` is `None`) or the R1
    /// judgement (`support::formula_backing`) calls the formula CLUELESS
    /// for this tool, operation and wood. The resolver [`feeds_support`]
    /// returns `FeedsSupport::Refuse` for it.
    ///
    /// This is a different refusal from `WrongToolForOperation`. That one
    /// says the tool cannot make the cut. This one says the engine has no
    /// source for a recipe. An add door still creates the operation with
    /// its defaults and no recipe (`suggest::default_operation`).
    Unbacked {
        operation: crate::compute::catalog::OperationType,
        tool_family: vendor_lut::ToolFamily,
        material: vendor_lut::MaterialFamily,
        reason: &'static str,
    },
}

impl std::fmt::Display for FeedsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedsError::WrongToolForOperation {
                operation,
                family,
                actual,
                allowed,
            } => {
                let op_name = operation
                    .map(|op| op.spec().label.to_owned())
                    .unwrap_or_else(|| format!("the {family:?} family"));
                let this = actual.label();
                let names: Vec<&str> = allowed.iter().map(|k| k.label()).collect();
                if let Some((last, init)) = names.split_last() {
                    let list = if init.is_empty() {
                        (*last).to_owned()
                    } else {
                        format!("{} or {last}", init.join(", "))
                    };
                    write!(f, "{op_name} needs a {list} tool; this tool is a {this}.")
                } else if *actual == CutterKind::VBit {
                    write!(f, "{op_name} does not accept a V-bit.")
                } else {
                    write!(
                        f,
                        "{op_name} with a scallop height needs a tool with a curved tip (ball, \
                         bull or tapered ball); this tool is a {this}."
                    )
                }
            }
            FeedsError::Unbacked {
                operation,
                tool_family,
                material,
                reason,
            } => write!(
                f,
                "Suggest has no basis for {} with a {} in {}: {reason}",
                operation.label(),
                tool_family.label(),
                material.label(),
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
/// Two engine rules refuse here (feeds matrix ruling R1, 2026-09-23):
///
/// 1. The operation's registry row does not allow the tool's cutter kind
///    (`OpRegistryEntry::tool_constraints`). The generator refuses on the
///    same predicate, so Suggest and generation cannot disagree. This
///    needs `operation_kind`; a raw `calculate` caller without one skips it.
/// 2. Scallop + Flat|VBit (no tip radius — the `R - √(R² - (s/2)²)`
///    scallop formula is undefined) and Parallel + Flat|VBit when
///    `target_scallop_mm.is_some()` (the DropCutter scallop hint routes
///    through the same scallop-stepover block in `calculate`).
///
/// After the tool check, it refuses with [`FeedsError::Unbacked`] when
/// [`feeds_support`] returns `FeedsSupport::Refuse`: no vendor row, and
/// either no formula source or a wood cell the R1 judgement calls
/// clueless (`support::formula_backing`). This is the one place that
/// constructs `Unbacked`.
pub fn validate_tool_for_operation(input: &FeedsInput) -> Result<(), FeedsError> {
    let actual = input.tool_geometry.cutter_kind();
    if let Some(operation) = input.operation_kind {
        let constraints = operation.registry_entry().tool_constraints;
        if !constraints.allows(actual) {
            return Err(FeedsError::WrongToolForOperation {
                operation: Some(operation),
                family: input.operation,
                actual,
                allowed: constraints.required_kinds,
            });
        }
    }
    let scallop_relevant = input.operation == OperationFamily::Scallop
        || (input.operation == OperationFamily::Parallel && input.target_scallop_mm.is_some());
    if scallop_relevant && matches!(actual, CutterKind::Flat | CutterKind::VBit) {
        return Err(FeedsError::WrongToolForOperation {
            operation: input.operation_kind,
            family: input.operation,
            actual,
            allowed: &[],
        });
    }
    if let Some(operation) = input.operation_kind
        && let FeedsSupport::Refuse { reason } = feeds_support(input)
    {
        return Err(FeedsError::Unbacked {
            operation,
            tool_family: input.tool_geometry.cutter_kind().lut_family(),
            material: vendor_normalize::material_to_lut(input.material).0,
            reason,
        });
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

/// Name the clamp [`effective_rubbing_floor`] would apply at this band —
/// the [`ClampReason`] Step-9b's warning and the explanation record both
/// describe. Checkpoint K (d2).
///
/// Pure naming: it applies nothing and decides nothing. Step-9b calls it
/// so the warning and the record cannot describe the clamp differently,
/// which is the failure mode rule 5 names.
#[must_use]
pub fn rubbing_floor_clamp_reason(band: Option<ChiploadBounds>) -> ClampReason {
    let floor = effective_rubbing_floor(band);
    if floor < RUBBING_FLOOR_MM_TOOTH {
        ClampReason::RubbingFloorCappedToBandCeiling {
            floor_mm_per_tooth: floor,
            global_floor_mm_per_tooth: RUBBING_FLOOR_MM_TOOTH,
        }
    } else {
        ClampReason::RubbingFloor {
            floor_mm_per_tooth: floor,
        }
    }
}

/// **Post-hoc: is this commanded advance sitting exactly where the
/// rubbing-floor clamp puts one?** Checkpoint K (c2)/(d2).
///
/// Asked by the post-simulation chipload gate, which sees a finished
/// recipe and not the Suggest run that produced it. It reads the same
/// [`effective_rubbing_floor`] the clamp applies — so it is one decision
/// consulted twice, not a mirror of Step-9b that can drift — and the
/// "sits on" test is [`crate::tool_load::boundary::is_at_bound`], the
/// same epsilon the gate's own comparisons use.
///
/// # What it cannot tell you, stated
///
/// It identifies the **operating point**, not its author. A feed an
/// operator typed by hand that happens to land on the identical value is
/// indistinguishable from a clamped one. For the consumer this exists
/// for — deciding whether a verdict *on the bound* should read `clamped`
/// rather than `exceeds` — that is the right answer either way: the
/// engine will not command past a band ceiling, so a recipe resting on
/// one is at the operating point the engine itself would choose.
///
/// The faithful channel is the recipe's own
/// [`FeedsWarning::ChiploadClampedToFloor`], which no shipped structure
/// carries from Suggest to the gate. Recorded as NOT EXERCISED in the
/// A-7 wave entry with that plumbing as the resume condition.
#[must_use]
pub fn recipe_parked_by_rubbing_floor(
    commanded_fpt_mm: f64,
    band: Option<ChiploadBounds>,
) -> Option<ClampReason> {
    if commanded_fpt_mm <= 0.0 || !commanded_fpt_mm.is_finite() {
        return None;
    }
    let floor = effective_rubbing_floor(band);
    crate::tool_load::boundary::is_at_bound(commanded_fpt_mm, floor)
        .then(|| rubbing_floor_clamp_reason(band))
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

/// Assemble the Suggest path's inputs to the canonical two-term power
/// model (`tool_load::power`), so Step 6's clamp and the published
/// `power_kw` are built from one expression rather than two.
///
/// `effective_d` must be the CHIP-THINNING diameter — the circle that
/// actually touches material at this DOC — because it sets both the
/// immersion angle ψ and the cutting velocity `Vc = π·D·n`. That is the
/// same binding `radial_chip_thinning_factor` reads, not the LUT-row
/// diameter this function's callers shadowed earlier.
fn power_model_terms(
    input: &FeedsInput,
    kc: f64,
    cross_section_mm2: f64,
    ap: f64,
    ae: f64,
    effective_d: f64,
    rpm: f64,
) -> crate::tool_load::power::PowerTerms {
    crate::tool_load::power::PowerTerms::of(crate::tool_load::power::PowerModelInputs {
        kc_n_per_mm2: kc,
        cross_section_mm2,
        axial_doc_mm: ap,
        // One immersion-angle definition for the whole engine: the same
        // `force::immersion_angle` the deflection cap and the modulator
        // consume.
        immersion_rad: force::immersion_angle(ae, effective_d / 2.0),
        engagement_diameter_mm: effective_d,
        spindle_rpm: rpm,
        flute_count: f64::from(input.flute_count),
    })
}

/// Main calculation entry point.
/// Smallest axial depth the power ladder may propose (mm).
///
/// Mirrors `suggest::DEFLECTION_BACKOFF_DPP_FLOOR_MM`, which bounds the
/// deflection back-off for the same reason: without a floor a solver that
/// cannot reach the budget walks the geometry to zero and ships a recipe that
/// removes no material. When the floor binds, the cut stays over budget and
/// the `PowerLimited` warning fires — the unmet-constraint signal, not a
/// fabricated pass.
const POWER_LADDER_AP_FLOOR_MM: f64 = 0.5;

/// Smallest radial width the power ladder may propose (mm). See
/// [`POWER_LADDER_AP_FLOOR_MM`].
const POWER_LADDER_AE_FLOOR_MM: f64 = 0.5;

/// Largest value in `[floor, current]` that satisfies `fits`, by bisection.
///
/// `fits` must be monotone: true at `floor` implies true everywhere below
/// `current`. Spindle power rises monotonically with both the axial depth and
/// the radial width, so both callers qualify.
///
/// Returns `None` when even `floor` does not fit, which is the caller's
/// signal that this rung cannot close the gap and the ladder must go on.
fn largest_fitting(current: f64, floor: f64, fits: impl Fn(f64) -> bool) -> Option<f64> {
    if !(current.is_finite() && floor.is_finite()) || current <= floor {
        return None;
    }
    if !fits(floor) {
        // Even the floor is over budget. Give everything this rung has
        // anyway and let the ladder carry the remainder: unlike the RPM
        // traverse, a PARTIAL geometry reduction has no downside beyond the
        // smaller cut the operator is already being told about, and it
        // lowers the feed cut the last rung has to make.
        return Some(floor);
    }
    let mut lo = floor;
    let mut hi = current;
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

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
    // — the chipload band's depth de-rate uses the same kind of diameter,
    // taken again at the shipped depth after the power ladder.
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
    // P1 sidecar (2026-08-22) — the band the RUBBING FLOOR is subordinated
    // to when the recipe resolver's row publishes no chipload column.
    //
    // `chipload_bounds` below comes from the **recipe** resolver, which lets
    // RPM-only anchors win. When one does, `bounds` is `None` and
    // `effective_rubbing_floor` falls back to the bare
    // `RUBBING_FLOOR_MM_TOOTH` — even where a chipload-bearing row exists and
    // the post-sim gate is about to judge this very cut against it. Measured
    // on a Ø1 tapered ball: Suggest's own model asked for 0.0093–0.0137
    // mm/tooth and the bare constant overrode it to 0.025, while
    // `amana-tapered-hardwood-parallel-3175-2f` sat underneath publishing
    // 0.010–0.020.
    //
    // So the floor consults the **envelope** resolver — the gate's, same
    // query, same DOC derate, same both-bounds policy — before falling back
    // to the constant. This introduces no new constant and no new exponent;
    // it hands an existing subordination rule the band it already had. It
    // deliberately does NOT become `chipload_bounds`: the recipe legitimately
    // rests on the RPM anchor, and re-pointing Suggest's *target* at another
    // row is a different change with a different justification.
    let mut floor_band_fallback_raw: Option<ChiploadBounds> = None;
    // The one recipe row lookup (`support::recipe_row_lookup`). The
    // support arm below reads the same result, so the arm and the row
    // cannot disagree.
    let row_lookup = support::recipe_row_lookup(input);
    if matches!(row_lookup, support::RecipeRowLookup::RoutingRefused) {
        // Checkpoint K (a4) — the routing REFUSED. Ruled in on
        // the Suggest side as well as the gate's: 378
        // recommendations that carried a confident vendor band on
        // a surface the gate declined to judge become honest
        // no-vendor-data. Say which rows are missing, not just
        // "no data".
        let tool_family = input.tool_geometry.cutter_kind().lut_family();
        warnings.push(FeedsWarning::NoVendorRowsForRoutedOperation {
            operation_kind: format!("{:?}", input.operation_kind),
            tool_family: format!("{tool_family:?}"),
            missing_rows: vendor_normalize::missing_project_curve_rows(tool_family).to_owned(),
        });
    }
    let support = support::support_for_lookup(input, &row_lookup);
    let (
        chip_load,
        vendor_rpm,
        vendor_rpm_max,
        vendor_rpm_min,
        vendor_source,
        chipload_source,
        chipload_band_raw,
    ) = if let support::RecipeRowLookup::Row {
        lut,
        query,
        row: result,
    } = row_lookup
    {
        let result = *result;
        matched_lut_row = Some(result.clone());
        let observation_id = result.observation_id;
        // Capture the LUT-derived chipload band (post diameter
        // /hardness scaling) for Suggest v2 step 2's feed-up
        // recalibration loop. Only populated when the row
        // publishes both bounds — partial-band rows (one side
        // only) leave it None so the loop doesn't fire on an
        // ambiguous target.
        //
        // The band here is the RAW row band: validated, not yet
        // de-rated. The axial depth that ships is not known until Step 3
        // and the power ladder have run, so the depth de-rate is applied
        // once, after them (see "The chipload band at the shipped depth"
        // below). Validation lives in `geometry::derate_chipload_bounds`
        // (S.8); a ratio of `0.0` gets a scale of `1.0`.
        let bounds = geometry::derate_chipload_bounds(
            result.chip_load_min_mm,
            result.chip_load_max_mm,
            0.0,
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
                result.rpm_min,
                Some(observation_id.clone()),
                ChiploadSource::VendorLut { observation_id },
                bounds,
            )
        } else {
            // Checkpoint K (a3) — disclose the fallback. The RPM
            // anchor is kept (that is why the two resolvers stay
            // separate); what was invisible until now is that the
            // chipload beside that row id is the empirical
            // formula's, and that this recommendation therefore
            // carries no band while the gate will judge it
            // against one.
            // P1: hand the floor the band the gate will use.
            let mut floor_band_row: Option<String> = None;
            if let Some(env) =
                vendor_lookup::find_best_chip_envelope_row(lut, &query, &input.tool_geometry)
                && let Some((min, max)) = geometry::derate_chipload_bounds(
                    env.chip_load_min_mm,
                    env.chip_load_max_mm,
                    0.0,
                    geometry::ChiploadBoundPolicy::RequireBoth,
                )
                .and_then(geometry::DeratedChiploadBand::into_pair)
            {
                floor_band_fallback_raw = Some(ChiploadBounds {
                    min_mm_per_tooth: min,
                    max_mm_per_tooth: max,
                });
                floor_band_row = Some(env.observation_id);
            }
            warnings.push(FeedsWarning::VendorRowPublishesNoChipload {
                observation_id: observation_id.clone(),
                formula_chipload_mm: formula_chipload,
                floor_band_from: floor_band_row,
            });
            (
                formula_chipload,
                result.rpm_nominal,
                result.rpm_max,
                result.rpm_min,
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
    // `spindle_scale` is captured into `FeedsDerates` for the modal.
    // Default (MatchChart) leaves it at 1.0; smoke baselines unchanged.
    let mut spindle_scale = 1.0_f64;
    let mut spindle_scale_reason = SpindleScaleReason::Unchanged;
    if matches!(input.spindle_strategy, SpindleStrategy::MaxSpeed) && rpm > 0.0 {
        let (_, machine_max_rpm) = machine.rpm_range();
        let machine_ceiling = machine_max_rpm * SPINDLE_CEILING_HEADROOM;
        // Diameter-tier literature ceiling for milling. The MaxSpeed
        // speedup pre-2026-06-03 used machine_ceiling unconditionally,
        // pushing a 12 mm hardwood adaptive2d cut to 22.8 k RPM —
        // ~63% above the Onsrud/Amana 14 k literature ceiling
        // (literature-matrix cell flat_12mm_adaptive2d_oak_power).
        // Drill ops already had a diameter tier from round-4
        // (`drill_rpm_envelope_for_diameter`, commit 9c4f3b9); this is
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
            spindle_scale = raw_speedup.min(MAX_SPINDLE_SPEEDUP);
            spindle_scale_reason = SpindleScaleReason::MaxSpeedPolicy;
            rpm = machine.clamp_rpm(rpm * spindle_scale);
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
                spindle_scale *= rpm / pre_clamp;
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
    // We also roll `spindle_scale` back proportionally so the modal
    // reports the actual speedup the engine kept, not the requested
    // one it then undid.
    if input.operation == OperationFamily::Drill {
        let (floor, ceil) = drill_rpm_envelope_for_diameter(d);
        let pre_clamp = rpm;
        rpm = rpm.clamp(floor, ceil);
        rpm = machine.clamp_rpm(rpm);
        if pre_clamp > 0.0 && rpm < pre_clamp {
            spindle_scale *= rpm / pre_clamp;
        }
    }

    // --- Step 3: DOC/WOC from operation defaults ---
    let profile = operation_default_profile(input.operation, input.pass_role);
    let (mut ap, mut ae) = default_engagement(d, &profile, input, machine);

    // --- Step 3b: Scallop-driven stepover for ball/tapered ball ---
    if let Some(target_scallop) = input.target_scallop_mm {
        // A bull nose forms its scallop with its corner radius (EVIDENCE
        // 2-7): the registry refuses Bull on the Scallop family, so this
        // arm serves the Parallel family's scallop hint.
        let ball_r = match input.tool_geometry {
            ToolGeometryHint::Ball => d / 2.0,
            ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
            ToolGeometryHint::Bull { corner_radius } => corner_radius,
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
    // band is de-rated by (at the shipped depth, before the rubbing floor). This is the value published as
    // `FeedsResult::effective_diameter_mm`.
    let effective_d = effective_diameter(
        input.tool_geometry,
        d,
        input.shank_diameter.unwrap_or(d),
        ap,
    );

    // ── Chip thinning: MEASURED, NOT APPLIED (G-CHIPTHIN-HALFFIX, 2026-08-19)
    //
    // These two factors are still computed, because the geometric condition
    // they describe is real and worth reporting — a Ø6 ball at 0.05 mm DOC
    // genuinely does present a thinner chip per tooth-pass. What was deleted
    // here is the **multiplication into the feed**.
    //
    // The reason is the same one that deleted the gate-side normalisation on
    // 2026-08-06, applied to the half that wave did not reach.
    // `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` §2.3 / N-8
    // records, as a deliberate negative result, that **no wood source in the
    // shipped LUT publishes a radial-engagement condition for its chipload
    // column**: six wood charts were read in full and all six state an *axial*
    // condition (1 × D) plus an axial derate table, none mentioning stepover,
    // width of cut, radial engagement or `ae`. A correction can only be applied
    // against a stated reference condition, and for these columns there is
    // none. The `ae_min`/`ae_max` values the factor read are repo-authored
    // application windows (`ae_rule` strings like `"scallop driven"`), not
    // transcriptions — so the multiplier was scaling a vendor number by a
    // quantity the vendor never conditioned it on.
    //
    // The same argument retires the axial term: the vendor charts publish a
    // rule for cutting *deeper* than 1 × D (reduce chipload) and no rule at all
    // for cutting shallower, so "feed faster at shallow DOC" has no source
    // either.
    //
    // Measured before the deletion (`tests/chipload_thinning_magnitude_survey.rs`,
    // 3 420 operating points): the multiplier was active on 99.1 % of them at a
    // median of 1.809, with 636 points pinned to the 4.0 clamp ceiling — i.e.
    // there the applied value was not the geometric one, it was the cap.
    // Against the derated vendor band it put 37.7 % of banded points ABOVE the
    // maximum the post-sim gate judges them by. Deleting it drops that to
    // 0.1 % and more than doubles in-band adherence, 19.0 % → 44.5 %, once the
    // Step-9b rubbing floor below catches the fall. Ruled by the operator
    // 2026-08-19 after that survey; the seed-target half of the question
    // (midpoint vs band maximum) was deliberately NOT bundled with it.
    //
    // Consequence worth knowing: with this gone, `ae` no longer enters the feed
    // expression at all except through the Step 6 power check, which
    // `power_ceiling_parity_f2.rs` measured as never firing on shipped
    // profiles. A stepover change therefore no longer moves the feed.
    let rctf = geometry::radial_chip_thinning_factor(ae, effective_d);
    let axial_thinning = match input.tool_geometry {
        ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
            geometry::axial_chip_thinning_factor_for_ball(d, effective_d)
        }
        _ => 1.0,
    };
    let chip_thinning = (rctf * axial_thinning).clamp(1.0, 4.0);

    // Depth de-rate on the feed. Onsrud, Freud and Amana print the same
    // three points (1 x D full, 2 x D minus 25 %, 3 x D minus 50 %; Freud
    // says "at least", Amana says "feed rate"), keyed to the AXIAL depth.
    // `depth_tier_multiplier` is `geometry::doc_derating_scale`, the one
    // scale that the chipload band and the post-sim gate also use (R3).
    //
    // Feeds matrix R2 (2026-09-23): on a tapered ball the diameter is the
    // engaged diameter at `ap`, the one the band below and the post-sim
    // chipload gate de-rate at. Before R2 the feed used the tip `d`, so
    // the feed and the band read two ratios on one tool. Every other shape
    // keeps `d`; see `geometry::feed_ladder_diameter_mm`.
    let ladder_d = geometry::feed_ladder_diameter_mm(
        input.tool_geometry,
        ap,
        d,
        input.shank_diameter.unwrap_or(d),
    );
    let depth_tier = geometry::depth_tier_multiplier(ap, ladder_d);

    let mut raw_feed = rpm * chip_load * input.flute_count as f64 * depth_tier;

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
    // `tool_load::power::PowerTerms` so the Suggest path and the Sim
    // verdict can't diverge. The cross-section uses the geometry-hint's
    // shape-correct area (V-bit triangular, flat/ball/bull/tapered
    // rectangular) — same contract as the cutter trait's
    // `mrr_cross_section_mm2` the Sim verdict reads — and it feeds the
    // SHEAR term only. R1's edge term reads `ap`, ψ, the engaged
    // diameter and the RPM instead; `power_model_terms` assembles both
    // halves from one place.
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
    // The CLAMP below states the COMMANDED form: it caps `raw_feed` so
    // that the feed Step 9 will actually command, `safety_factor ·
    // raw_feed`, satisfies the gate's `required <= power_at_rpm ·
    // safety_factor`. Steps 7/9c only reduce the feed further, so the
    // guarantee survives them.
    //
    // R1 (2026-09-16) is why the form is written out. Pre-R1 power was
    // linear in feed, so clamping the RAW feed against the unfactored
    // `power_at_rpm` gave the same answer for free — Step 9's scale
    // carried through the model. The two-term model's edge term carries
    // no feed, so `P(safety_factor · f) != safety_factor · P(f)` and the
    // composition had to stop being implicit. With no edge term the two
    // expressions are algebraically identical, which is why this is a
    // restatement rather than a new derate.
    //
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
    // Measured (tests/power_ceiling_parity_f2.rs,
    // `the_power_ceiling_binds_on_three_shipped_fixtures`): across all
    // three shipped presets × ten species × Ø3/Ø6/Ø12 slots, peak
    // utilisation is 80 % and the branch fires on three of the ninety
    // fixtures — Ø12 slots in Jarrah and Ipe. Before R1 (2026-09-16) the
    // pre-R1 linear power model put that peak at 23.6 % and the branch
    // never fired at all; the edge term is what made it live, and it is
    // largest exactly here, where a full-width slot runs the duty cycle
    // `z·ψ/2π` at its maximum of 1.
    let available_power = machine.power_at_rpm(rpm);
    // The gate's ceiling (`power.rs:214`) — what every published power
    // number below is quoted against.
    let gate_available_power = available_power * machine.safety_factor;
    let mut power_limited = false;
    let mut feed = raw_feed;
    let mut power_factor = 1.0;
    // Power-ladder bookkeeping. An RPM move is booked on `spindle_scale`, a
    // geometry move changes `ap` / `ae` themselves, and only the last-resort
    // feed rung touches `power_factor` — the CHIPLOAD axis — because only
    // that rung changes the advance per tooth.
    let rpm_before_ladder = rpm;
    let mut ladder_moved_rpm = false;
    let mut ladder_ap_from: Option<f64> = None;
    let mut ladder_ae_from: Option<f64> = None;
    let mut ladder_feed_factor: Option<f64> = None;

    if let Some(kc) = material.kc_n_per_mm2()
        && available_power > 0.0
    {
        let cross_section = input.tool_geometry.mrr_cross_section_mm2(ap, ae);
        let terms = power_model_terms(input, kc, cross_section, ap, ae, effective_d, rpm);
        // R1 — the clamp is stated on the COMMANDED axis.
        //
        // Pre-R1 power was linear in feed, so clamping `raw_feed`
        // against the unfactored `power_at_rpm` and letting Step 9's
        // `safety_factor` scale the feed landed the commanded power
        // exactly on the gate's `power_at_rpm × safety_factor`. That
        // composition was linearity doing the work for free, and the
        // two-term model does not have it: the edge term carries no
        // feed, so scaling the feed by `safety_factor` does NOT scale
        // the power by `safety_factor`.
        //
        // So the same statement is now made explicitly — cap `raw_feed`
        // such that the feed Step 9 will actually command,
        // `safety_factor × raw_feed`, draws no more than the gate's
        // ceiling. With no edge term this reduces algebraically to the
        // pre-R1 clamp, which is why the two axes above still describe
        // the same guarantee.
        let sf = machine.safety_factor.max(1e-9);
        let required_at_commanded = terms.kw_at_feed(sf * raw_feed);
        if required_at_commanded > gate_available_power {
            // `feed_for_kw` answers on the commanded axis; divide back
            // out to get the raw-axis cap Step 9 will scale.
            //
            // It returns `None` when the feed-free edge term ALONE is
            // over the budget — the power-side twin of
            // `DeflectionCapRefusal::EdgeForceOverBudget`. No feed
            // rescues that cut: thinning the chip leaves the ploughing
            // power exactly where it was, while driving the chipload
            // toward the rubbing floor and raising the energy spent per
            // mm³. So the feed is left alone and the warning carries the
            // conflict — the same clamp-and-warn convention as the
            // rubbing floor at Step 9b, which also refuses to serve a
            // recipe it cannot honour. The real fix is less DOC, less
            // stepover or a lower RPM, none of which a feed derate can
            // reach for. Serving a zero feed instead would be a
            // fabricated recipe, not an answer.
            // ── The engagement ladder (2026-09-16) ────────────────────
            //
            // Pre-ladder this scaled the feed and nothing else. That is
            // the wrong dial for this constraint, twice over.
            //
            // First, the edge term carries no feed at all, so thinning
            // the chip sheds only part of the load while pushing the
            // chipload toward the rubbing floor. On the motivating case
            // — Ø12 4-flute bull, white oak, Shapeoko VFD at 9 000 rpm —
            // the feed derate came out at 0.058, a 17x cut, and the
            // recipe was STILL over budget afterwards and rubbing-
            // adjacent. Two warnings fired and neither constraint was
            // honoured. That is literature-matrix cell
            // `bull_12mm_pocket_oak`.
            //
            // Second, what actually sheds spindle load depends on the
            // spindle. On a constant-torque VFD below its rated speed
            // the available power falls with the RPM at exactly the rate
            // the required power falls, so walking the constant-chipload
            // line changes NOTHING: measured 120 % utilisation at 9 000,
            // 6 000, 4 500 and 3 000 rpm alike. On a constant-power
            // router the same walk takes 95 % to 32 %.
            //
            // So the ladder asks what this cut can give up, in order,
            // and stops at the first rung that works:
            //
            //   1. RPM, holding the chipload — where it helps.
            //   2. Axial depth — where the ENGINE chose it.
            //   3. Radial width — where the ENGINE chose it.
            //   4. Nothing. Leave the cut alone and say so.
            //
            // Rungs 2 and 3 do not move the feed or the RPM, so the
            // advance per tooth is untouched: the cut gets smaller, not
            // slower and thinner.
            //
            // Rung 1 branches on MEASURED behaviour, not on the
            // `PowerModel` variant. The physical question is "does a
            // lower RPM reduce utilisation here", and the numbers answer
            // it directly — which also covers the flat region above a
            // VFD's rated speed without a special case, and keeps
            // working if a new `PowerModel` is added.
            //
            // See planning/load_model_2026-09-16/{DERATE_SPEC.md,
            // IMPLEMENTATION_PLAN.md} and derate_levers.py.

            // Required kW at a candidate operating point, stated on the
            // COMMANDED axis so it compares like-for-like with the budget.
            let required_at = |rpm_c: f64, ap_c: f64, ae_c: f64, feed_c: f64| -> f64 {
                let cs = input.tool_geometry.mrr_cross_section_mm2(ap_c, ae_c);
                power_model_terms(input, kc, cs, ap_c, ae_c, effective_d, rpm_c)
                    .kw_at_feed(sf * feed_c)
            };
            let budget_at =
                |rpm_c: f64| machine.power_at_rpm(rpm_c).max(0.0) * machine.safety_factor;

            // ── Rung 1: walk the constant-chipload line ───────────────
            //
            // At a fixed chipload the feed is proportional to the RPM, so
            // a candidate RPM implies its own feed.
            // The traverse has a FLOOR, and it is not the machine's.
            //
            // A published RPM band is a statement about the cut, not about
            // the machine: below its minimum the surface speed is too low
            // for the material and the tool rubs or builds up an edge —
            // acutely so in aluminium. Step 2b already honours the same
            // row's `rpm_max` on the way up; this is the same rule facing
            // the other way.
            //
            // The literature matrix found this: the first version of the
            // ladder bounded only by `machine.rpm_range()` and walked
            // `flat_6mm_pocket_al6061_lut` from 20 000 rpm to 8 000, which
            // is 55 % under the row's published minimum. Shedding spindle
            // load by cutting aluminium at a wood RPM is not a trade the
            // engine gets to make silently.
            let machine_floor = machine.rpm_range().0;
            let rpm_floor = match vendor_rpm_min {
                Some(v) if v.is_finite() && v > 0.0 => machine_floor.max(v),
                _ => machine_floor,
            };
            let chipload_held = if rpm > 0.0 { raw_feed / rpm } else { 0.0 };
            let feed_at = |rpm_c: f64| chipload_held * rpm_c;

            let fits = |rpm_c: f64, ap_c: f64, ae_c: f64| -> bool {
                required_at(rpm_c, ap_c, ae_c, feed_at(rpm_c)) <= budget_at(rpm_c)
            };

            // Does slowing down help AT ALL? Ask the slowest speed this
            // spindle can run. On a constant-torque VFD the answer is no,
            // and the ladder must not waste the operator's RPM on it.
            let slowest = machine.next_rpm_at_or_below(rpm_floor);
            let util_now = required_at_commanded / gate_available_power.max(1e-12);
            let util_slowest = {
                let b = budget_at(slowest);
                if b > 0.0 {
                    required_at(slowest, ap, ae, feed_at(slowest)) / b
                } else {
                    f64::INFINITY
                }
            };
            // 1 % of utilisation is the threshold for "this lever does
            // something". The VFD case lands at exactly 0 % by
            // construction; a router moves tens of percent.
            const TRAVERSE_HELPS_EPS: f64 = 0.01;
            let traverse_helps = util_slowest < util_now - TRAVERSE_HELPS_EPS;

            // Use the traverse ONLY when it can close the gap by itself.
            //
            // The first version bisected regardless, so when the traverse
            // could not close the gap the bisection converged on the floor
            // and the ladder spent the machine's entire speed range for a
            // partial gain — and then shrank the cut anyway. The literature
            // matrix showed the result plainly: two unrelated cells both
            // pinned at exactly 8 000 rpm, the floor, for different reasons.
            //
            // A partial traverse is not obviously better than no traverse:
            // both cost time, and a slower spindle also costs surface
            // finish and tool life. So the rule is all-or-nothing, and when
            // the RPM cannot do the job the geometry does it at the RPM the
            // chart chose.
            let traverse_can_close_it = fits(rpm_floor, ap, ae);
            if traverse_helps && traverse_can_close_it && rpm > rpm_floor {
                // Highest RPM that fits, by bisection on the continuous
                // range, then rounded DOWN to a speed the spindle can
                // actually run. Rounding down matters: `clamp_rpm` snaps
                // to the NEAREST speed and would round back up, raising
                // the load this rung exists to shed.
                let mut lo = rpm_floor;
                let mut hi = rpm;
                for _ in 0..40 {
                    let mid = 0.5 * (lo + hi);
                    if fits(mid, ap, ae) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let candidate = machine.next_rpm_at_or_below(lo);
                if candidate < rpm && candidate > 0.0 {
                    // Book it on the SPEED axis. The chipload did not
                    // change, so `combined_factor` must not see this.
                    spindle_scale *= candidate / rpm;
                    spindle_scale_reason = SpindleScaleReason::PowerLimit;
                    raw_feed = feed_at(candidate);
                    rpm = candidate;
                    feed = raw_feed;
                    ladder_moved_rpm = true;
                }
            }

            // ── Rungs 2 and 3: make the cut smaller ───────────────────
            //
            // The feed and the RPM are settled now. Power rises
            // monotonically with both the axial depth and the radial
            // width, so the largest value that fits is a bisection.
            //
            // A knob the USER pinned is not the engine's to move. Pinning
            // a depth is a statement about the part, and quietly cutting
            // it shallower would change what was asked for rather than
            // how fast it is cut.
            if required_at(rpm, ap, ae, raw_feed) > budget_at(rpm)
                && input.axial_depth_mm.is_none()
                && let Some(fitted) = largest_fitting(ap, POWER_LADDER_AP_FLOOR_MM, |c| {
                    required_at(rpm, c, ae, raw_feed) <= budget_at(rpm)
                })
                && fitted < ap
            {
                ladder_ap_from = Some(ap);
                ap = fitted;
            }
            if required_at(rpm, ap, ae, raw_feed) > budget_at(rpm)
                && input.radial_width_mm.is_none()
                && let Some(fitted) = largest_fitting(ae, POWER_LADDER_AE_FLOOR_MM, |c| {
                    required_at(rpm, ap, c, raw_feed) <= budget_at(rpm)
                })
                && fitted < ae
            {
                ladder_ae_from = Some(ae);
                ae = fitted;
            }

            // ── Rung 4: the feed, LAST ────────────────────────────────
            //
            // The feed is a poor lever for this constraint, not a forbidden
            // one. The edge term carries no feed, so a feed cut sheds only
            // part of the load and drives the chipload toward rubbing — which
            // is why it must not be FIRST. But shipping a recipe the spindle
            // cannot turn is worse than shipping a thinner chip, and the
            // gate's contract is that a power-limited recommendation lands ON
            // the ceiling, never above it (`power_ceiling_parity_f2`).
            //
            // By the time the ladder reaches here the RPM and the geometry
            // have already taken the load down, so the feed cut needed is far
            // smaller than the pre-ladder one — the motivating case went from
            // a 17x feed cut to none at all. This rung exists to close the
            // last sliver, not to carry the constraint.
            //
            // It is booked on the CHIPLOAD axis, because unlike every rung
            // above it this one DOES change the advance per tooth.
            let mut still_required = required_at(rpm, ap, ae, raw_feed);
            let still_available = budget_at(rpm);
            if still_required > still_available {
                let terms_now = power_model_terms(
                    input,
                    kc,
                    input.tool_geometry.mrr_cross_section_mm2(ap, ae),
                    ap,
                    ae,
                    effective_d,
                    rpm,
                );
                // `None` means the feed-free edge term ALONE is over budget.
                // No feed rescues that cut: thinning the chip leaves the
                // ploughing power exactly where it was while driving the
                // chipload toward the rubbing floor. Leave the feed and let
                // the warning carry the conflict — the same clamp-and-warn
                // convention as the rubbing floor at Step 9b.
                if let Some(commanded_cap) = terms_now.feed_for_kw(still_available)
                    && raw_feed > 0.0
                {
                    let f = (commanded_cap / sf / raw_feed).clamp(0.0, 1.0);
                    if f < 1.0 {
                        power_factor = f;
                        raw_feed *= f;
                        ladder_feed_factor = Some(f);
                        still_required = required_at(rpm, ap, ae, raw_feed);
                    }
                }
                feed = raw_feed;
            }

            // ── Rung 5: report ────────────────────────────────────────
            //
            // `PowerLimited` means "the power constraint bound". It carries
            // the BEFORE pair, which is what it carried pre-ladder and what
            // makes it self-explaining: `required > available` always holds,
            // so the warning states the conflict that caused the change
            // rather than the state after it. `power_ceiling_parity_f2`
            // asserts exactly that.
            //
            // What the engine DID about it, and whether the cut fits now,
            // is `PowerLadderReducedCut`'s job. Splitting them this way
            // keeps each warning true on its own terms: one names the
            // constraint, the other names the response.
            power_limited = true;
            warnings.push(FeedsWarning::PowerLimited {
                required_kw: required_at_commanded,
                available_kw: gate_available_power,
            });
            if ladder_moved_rpm
                || ladder_ap_from.is_some()
                || ladder_ae_from.is_some()
                || ladder_feed_factor.is_some()
            {
                warnings.push(FeedsWarning::PowerLadderReducedCut {
                    rpm_from: ladder_moved_rpm.then_some(rpm_before_ladder),
                    rpm_to: ladder_moved_rpm.then_some(rpm),
                    axial_from: ladder_ap_from,
                    axial_to: ladder_ap_from.map(|_| ap),
                    radial_from: ladder_ae_from,
                    radial_to: ladder_ae_from.map(|_| ae),
                    feed_factor: ladder_feed_factor,
                    required_kw_before: required_at_commanded,
                    required_kw_after: still_required,
                    available_kw: still_available,
                });
            }
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
    // The CUTTING feed ceiling of Step 7 is treated as a hard physical
    // limit. This lift runs after Step 9, so it works on the COMMANDED
    // axis and the cap it reads is the ceiling on that axis:
    // `machine.commanded_cutting_feed_ceiling_mm_min()`. T-18 (2026-09-18)
    // replaced `machine.max_feed_mm_min × safety_factor` here — a fraction
    // of the gantry TRAVEL rate, which is a different quantity and lets the
    // lift restore a feed above the ceiling Step 7 just enforced. If
    // lifting feed to the floor would exceed that cap, we leave feed at the
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
    //
    // P1 (2026-08-22): the band handed to `effective_rubbing_floor` is the
    // recipe resolver's when it published one, and otherwise the **envelope**
    // resolver's — the gate's own row, resolved above with the same query and
    // the same DOC derate. Before this, an RPM-only recipe anchor meant the
    // floor saw no band at all and applied the bare constant, which on the
    // shipped tapered-ball rows is above every chipload the vendor prints
    // (all 8 of them; Ø6 → 0.018 against a 0.025 floor). The clamp reason is
    // derived from the SAME band, so the warning and the explanation record
    // cannot name a ceiling the floor did not use.
    // ── The chipload band at the shipped depth (feeds matrix R3) ──────
    //
    // The band that this function returns is the band at the axial depth
    // that ships: `geometry::doc_derating_scale(ap / D_lut)`, where `D_lut`
    // is the LUT-semantics engaged diameter at that depth. That is the
    // scale and the diameter that the post-sim chipload gate applies at the
    // measured depth, so Suggest's band, the UI's band and the gate's band
    // are one number. Before R3 the band was de-rated at the depth HINT
    // (or at 1 x D when no hint came), so every roughing cell showed the
    // raw row band (EVIDENCE 5.2-7). The rubbing floor below reads this
    // band too, which is the band the gate will use (P1).
    //
    // Drill ops are excluded, as in the gate (`NotApplicableForOp`):
    // a ratio of `0.0` gets a scale of `1.0`.
    let band_doc_ratio = if input.operation == OperationFamily::Drill {
        0.0
    } else {
        let band_d = input.tool_geometry.engaged_diameter_at_doc(
            ap.max(0.0),
            d,
            input.shank_diameter.unwrap_or(d),
        );
        if band_d > 0.0 { ap / band_d } else { 0.0 }
    };
    let band_at_depth = |raw: ChiploadBounds| {
        geometry::derate_chipload_bounds(
            Some(raw.min_mm_per_tooth),
            Some(raw.max_mm_per_tooth),
            band_doc_ratio,
            geometry::ChiploadBoundPolicy::RequireBoth,
        )
        .and_then(geometry::DeratedChiploadBand::into_pair)
        .map(|(min, max)| ChiploadBounds {
            min_mm_per_tooth: min,
            max_mm_per_tooth: max,
        })
    };
    let chipload_bounds = chipload_band_raw.and_then(band_at_depth);
    let floor_band_fallback = floor_band_fallback_raw.and_then(band_at_depth);

    let fpt_divisor = rpm * input.flute_count as f64;
    if fpt_divisor > 0.0 {
        let commanded_fpt = feed / fpt_divisor;
        let floor_band = chipload_bounds.or(floor_band_fallback);
        let floor = effective_rubbing_floor(floor_band);
        if commanded_fpt > 0.0 && commanded_fpt < floor {
            // Checkpoint K (d2) — the warning's `band_capped_from` and
            // the explanation record's `clamped_to` are now derived from
            // ONE naming function, so the two surfaces cannot describe
            // this clamp differently.
            let reason = rubbing_floor_clamp_reason(floor_band);
            warnings.push(FeedsWarning::ChiploadClampedToFloor {
                requested: commanded_fpt,
                floor,
                band_capped_from: match reason {
                    ClampReason::RubbingFloorCappedToBandCeiling {
                        global_floor_mm_per_tooth,
                        ..
                    } => Some(global_floor_mm_per_tooth),
                    ClampReason::RubbingFloor { .. } => None,
                },
            });
            let commanded_cut_ceiling = machine.commanded_cutting_feed_ceiling_mm_min();
            let target_feed = floor * fpt_divisor;
            feed = target_feed.min(commanded_cut_ceiling);
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
    //    The COMMANDED cutting-feed ceiling still wins over the envelope
    //    floor: a machine that can't reach the floor gets the honest
    //    conflict via the warning rather than an unreachable feed. This
    //    clamp runs after Step 9, so it works on the COMMANDED axis and
    //    the cap it reads is
    //    `machine.commanded_cutting_feed_ceiling_mm_min()` — the Step 7
    //    ceiling on that axis. T-18 (2026-09-18) replaced
    //    `machine.max_feed_mm_min × safety_factor` here, which is a
    //    fraction of the gantry TRAVEL rate and a different quantity.
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
            let commanded_cut_ceiling = machine.commanded_cutting_feed_ceiling_mm_min();
            let requested = feed;
            let clamped = feed.clamp(env_lo, env_hi).min(commanded_cut_ceiling);
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
                        spindle_scale *= target_rpm / rpm;
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
            power_model_terms(input, kc, cross_section, ap, ae, effective_d, rpm).kw_at_feed(feed)
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
        observed_radial_chip_thinning: rctf,
        observed_axial_chip_thinning: axial_thinning,
        observed_combined_chip_thinning: chip_thinning,
        depth_tier,
        ld_overhang: ld_factor,
        workholding: workholding_factor,
        power_limit: power_factor,
        feed_clamp: feed_clamp_factor,
        safety_factor: machine.safety_factor,
        spindle_scale,
        spindle_scale_reason,
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
        support,
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
///
/// Made `pub` on 2026-08-19 for Suggest pass 9's regression sentry
/// (`tests/suggest_feed_matches_final_geometry.rs`), which reconstructs the
/// calculator's geometry-dependent feed terms at two operating points. The
/// sentry deliberately calls THIS function rather than carrying a copy — a
/// second implementation of the chip-thinning diameter is exactly what C3
/// retired above.
pub fn effective_diameter(geom: ToolGeometryHint, nominal_d: f64, shank_d: f64, ap: f64) -> f64 {
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
mod tests;
