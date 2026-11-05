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

mod calculator;
pub mod cutter_constraints;
pub mod efficiency;
pub mod engagement;
pub mod envelope;
pub mod explain_payload;
pub mod extrapolation;
pub mod feed_explanation;
pub mod force;
pub mod force_budget;
pub mod geometry;
pub mod geometry_class;
pub mod lock;
pub mod operating_point;
pub mod plunge;
pub mod predict;
pub mod profile;
pub mod provenance;
pub mod quantities;
pub mod ramp;
pub mod rationale;
pub mod series;
pub mod sourced;
pub mod suggest;
pub mod support;
pub mod vendor_lookup;
pub mod vendor_lut;
pub mod vendor_normalize;
pub(crate) use calculator::{DepthClamps, StepRecording, calculate_with};
pub use efficiency::{ChipVerdict, CutEfficiency, cut_efficiency};
pub use explain_payload::{FeedsExplain, MachineEnvelope, explain as explain_feeds};
pub use feed_explanation::{
    ADVANCE_PER_TOOTH, AchievedFeedStage, CommandedStage, FeedExplanation, GateObservationStage,
    LutBandStage, ObservedStatistic,
};
pub use lock::{FeedsLock, SpeedEdit, SpeedField, SpeedRanges, SpeedWrite};
pub use operating_point::{
    PowerFigure, PowerUnmodeled, power_at_operating_point, power_at_shipped_point,
};
pub use plunge::{PlungeBasis, PlungeBinding};
pub use predict::{
    DeflectionBreakdown, DeflectionCaveat, DeflectionPrediction, DeflectionUnmodeled,
    predict_peak_deflection_um,
};
pub use provenance::{FeedsField, FeedsProvenance, ProvenanceSource, ValueProvenance};
pub use quantities::{
    ACHIEVED_ADVANCE_PER_TOOTH, ACHIEVED_PEAK_CHIP, ADVANCE_PER_TOOTH_UNIT,
    ARC_MEAN_CHIP_THICKNESS, AchievedFeedMmMin, AdvancePerToothMm, ArcMeanChipThicknessMm,
    COMMANDED_ADVANCE_PER_TOOTH, ChiploadBandClass, CommandedFeedMmMin, VendorChiploadBand,
};
pub use ramp::{EntryGeometry, RampBasis, RampFallback, RampFeed};
pub use sourced::{Basis, PointBases, Sourced};
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
use crate::material::{ConstantSource, FormulaConstant, MATERIAL_FORMULA_CONSTANTS, Material};

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
    /// Since ruling A1 (2026-09-24) the vendor-LUT lookup does not use
    /// this diameter for a tapered ball: the row is read at the tip
    /// ([`geometry::lut_key_diameter_mm`]). Since ruling B4 (2026-09-25)
    /// the lookup does not use it for a V-bit either: the row is read at
    /// the nominal diameter, or at its printed angle.
    ///
    /// For a tapered ball the depth cap still uses this engaged diameter
    /// ([`geometry::depth_derate_diameter_for_cutter`]). For a V-bit, B4
    /// step 2 moves those three to the nominal diameter too; only the
    /// surface-speed RPM and the formula chip load still use this engaged
    /// diameter.
    ///
    /// The hint-level twin of [`crate::tool::MillingCutter::lookup_diameter_at`].
    /// This path is called from `feeds::calculate` / `vendor_normalize`,
    /// which carry only a `ToolGeometryHint`, not a cutter. F075: the width
    /// math is not copied; both sides call the free functions
    /// [`crate::tool::ball_cone_radius_at_height`] and
    /// [`crate::tool::vbit_cone_radius_at_height`]. The clamps stay on each
    /// side, and the parity sentry
    /// `tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`
    /// holds them equal. If that test fails, do not silently prefer one side:
    /// Suggest's chipload-band derating and the post-sim gate's derating
    /// have split (planning/finishing_stack_review_2026-07.md S.3).
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
                // A hint has no constructor to refuse a degenerate shape, so
                // it keeps this guard.
                if tip_radius <= 0.0 || tan_alpha <= 0.0 {
                    return tool_diameter_mm;
                }
                // F075: the one ball-cone width, shared with the cutter.
                let radius = crate::tool::ball_cone_radius_at_height(
                    tip_radius, sin_alpha, cos_alpha, tan_alpha, axial_doc,
                );
                (2.0 * radius).clamp(0.0, shank_diameter_mm)
            }
            ToolGeometryHint::VBit { included_angle, .. } => {
                let half = (included_angle * 0.5).to_radians();
                // F075: the one cone width, shared with the cutter.
                (2.0 * crate::tool::vbit_cone_radius_at_height(half.tan(), axial_doc))
                    .clamp(0.0, tool_diameter_mm)
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

    pub const ALL: &[CutterKind] = &crate::all_variants!(CutterKind:
        Flat, Ball, Bull, VBit, TaperedBall,
    );

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
///
/// **The workholding rigidity is gone (ruling R4 Q8, 2026-09-24).** It was an
/// unsourced feed scale (0.85 / 1.00 / 1.03). Machine rigidity is
/// `MachineProfile::rigidity`.
#[derive(Default)]
pub struct SetupContext {
    /// Tool overhang from collet face (mm). Used for L/D derate.
    pub tool_overhang_mm: Option<f64>,
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
    /// The tool's cutter series (operator ruling 2026-10-31,
    /// [`series::ToolSeries`]): `None` is a generic (plain upcut) tool, and
    /// the vendor lookup reads the standard rows first. The product builder
    /// (`suggest::feeds_input_for_operation`) reads
    /// `ToolConfig::cut_direction`.
    pub tool_series: Option<series::ToolSeries>,
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
    /// K7 (feeds contract P9): the cases of the pre-sim power set. Step 6
    /// and the published `power_kw` read the maximum over them. The
    /// production builder (`suggest::feeds_input_for_operation`) reads the
    /// op's `CutSpec`; a caller with no op states
    /// [`engagement::PowerCases::NOMINAL`].
    pub power_cases: engagement::PowerCases,
    /// P13a (review H3): the kinds of the tool facts that the calculator
    /// reads. An `Unknown` fact plans at its legacy row, and the basis of
    /// each value that reads the fact names that row. The production
    /// builder reads the tool (`sourced::ToolFactKinds::of_tool`).
    pub tool_facts: sourced::ToolFactKinds,
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
    /// band floor.
    pub min_mm_per_tooth: f64,
    /// LUT-row chipload upper bound, post diameter/hardness scaling
    /// (mm/tooth). Suggest's feed-up loop uses this as the
    /// band ceiling; the midpoint is the chipload the recipe ships.
    pub max_mm_per_tooth: f64,
}

/// Result of the feeds calculation.
#[derive(Debug, Clone)]
pub struct FeedsResult {
    pub rpm: f64,
    pub chip_load_mm: f64,
    pub feed_rate_mm_min: f64,
    pub plunge_rate_mm_min: f64,
    /// The part of the plunge that does not use F (G10): the claim, or the
    /// named repo rule, and the tip cap. The apply funnel runs it again at
    /// the shipped feed ([`plunge::resolve_plunge`]).
    pub plunge: plunge::PlungeBasis,
    /// The sourced half of the ramp feed (G6 ramp): the axial chip of the
    /// drill claim, or why there is none. The number needs θ and the
    /// shipped feed and RPM, so `suggest::apply` resolves it
    /// ([`ramp::resolve_ramp_feed`]).
    pub ramp: ramp::RampBasis,
    pub axial_depth_mm: f64,
    pub radial_width_mm: f64,
    /// Predicted spindle power (kW) **at the CALCULATOR's geometry** — the
    /// `ap`, `ae`, RPM and feed this result carries, on the COMMANDED axis.
    ///
    /// It is not the power the machine draws. `suggest::invariants::enforce_invariants_at_fixed_geometry`
    /// runs after the calculator and may lower the depth per pass (the
    /// rigidity, cutting-length and deflection clamps) and the stepover, and
    /// pass 9 then re-derives the feed at the point that ships. A display of
    /// the SHIPPED figure calls
    /// [`crate::feeds::power_at_operating_point`] on the final operation.
    pub power_kw: f64,
    /// K7 (feeds contract P9): the case of the pre-sim power set that
    /// `power_kw` is the power of (the maximum over the cases that
    /// [`FeedsInput::power_cases`] admits).
    pub power_case: engagement::PowerCase,
    /// The ceiling `power_kw` is quoted against (kW): the rated spindle
    /// curve `power_at_rpm(rpm)`, the gate's own ceiling (ruling R4 Q2: no
    /// fraction).
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
    /// P11b (L6, chip first): the printed band at every depth. The vendor
    /// depth table is a limit of the envelope, not a scale on this band.
    /// The band holds the PEAK chip (L1).
    ///
    /// Until 2026-08-13 this was also the input to Suggest's chipload-aware
    /// feed-up recalibration (pass 8), which is retired — see the retirement
    /// note in [`crate::feeds::predict`]. The band still drives the
    /// rationale, the chipload envelopes and the post-sim gate; it no longer
    /// moves a feed before simulation.
    pub chipload_bounds: Option<ChiploadBounds>,
    /// The one printed chipload of a point row (A2, point mode), in
    /// mm/tooth. The row prints a maximum only, or two equal limits
    /// (`LookupResult::printed_chipload`). P11b: no depth de-rate, as
    /// [`Self::chipload_bounds`]. At most one of the two is
    /// `Some`: a point never becomes a band.
    pub chipload_point_mm: Option<f64>,
    /// Matched vendor row (post-scaling) from the LUT lookup, when one
    /// was found. Cloned through so the Suggest orchestrator's axial-DOC
    /// envelope pass ([`crate::feeds::cutter_constraints`]) and the
    /// chipload-bounds re-derivation step in `enforce_invariants_at_fixed_geometry` can
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
    /// CONTRACT K2 (feeds contract P5): the basis of each shipped field.
    /// Each step of [`calculate`] that writes a field writes its basis in
    /// the same statement. [`FeedsResult::provenance`] is a projection of
    /// these bases.
    pub bases: PointBases,
    /// P13b (K8, ruling Q7): one record per step of [`calculate`] that
    /// changed a value, in step order. The last record of a quantity holds
    /// the value of its result field.
    pub steps: Vec<crate::trace::lineage::StepRecord>,
}

/// Per-step record of the chipload → feed pipeline. Each multiplier
/// is positive (no zero divisors); a value of 1.0 means "no effect."
/// The "effective chipload" the toolpath actually cuts at is
/// `target_chip_load_mm × combined_factor()` — note the accessor, **not**
/// the product of every field. `observed_axial_chip_thinning` and
/// `ld_overhang` are records the engine reports but does not apply, so
/// multiplying the struct out by hand overstates the feed.
/// `combined_factor()` is the only correct composition.
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
    /// The radial chip-thinning lift (>= 1.0) that the feed takes, so that
    /// the peak chip is the band chip (feeds contract P11b, L1;
    /// [`geometry::peak_chip_thinning_lift`]). 1.0 at a width of half the
    /// engaged diameter or more, and on a finish pass (L2). Part of
    /// [`Self::combined_factor`].
    pub radial_chip_thinning: f64,
    /// Axial chip-thinning factor for ball / tapered-ball tools at shallow
    /// DOC: **OBSERVED, NOT APPLIED**. The vendor charts print a rule for a
    /// depth below 1 x D nowhere, so the feed does not take it.
    pub observed_axial_chip_thinning: f64,
    /// The long-tool (L/D) share of the load target (≤ 1.0): 0.88 above
    /// 4 x D stickout, 0.75 above 6 x D (repo rule, unsourced).
    ///
    /// **Not a feed factor since ruling R4 Q7 (2026-09-24).** The feed does
    /// not take it; the share is a reported record. It is therefore NOT part
    /// of [`Self::combined_factor`]. The Suggest dial pass that read it is
    /// deleted (P5), and D9 (2026-11-04) removed the machine dial.
    pub ld_overhang: f64,
    /// Power-limit factor (≤ 1.0). Applied when the calc had to back
    /// off feed to stay within the spindle's power envelope.
    pub power_limit: f64,
    /// Machine-feed-cap factor (≤ 1.0). Applied when the calc hit the
    /// machine's `max_feed_mm_min`.
    pub feed_clamp: f64,
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

/// The card and finding text of [`FeedsWarning::RpmLoweredForFeedCeiling`]:
/// "RPM lowered from 18000 to 15750 to keep the chip at the 4000 mm/min
/// feed limit of the machine". When the chip is not held in full it names
/// the stop.
#[must_use]
pub fn rpm_lowered_text(
    rpm_from: f64,
    rpm_to: f64,
    feed_ceiling_mm_min: f64,
    rpm_floor: f64,
    floor_source: RpmFloorSource,
    held: bool,
    arms: &CapArms,
) -> String {
    format!(
        "{}; {}",
        rpm_lowered_head(
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            floor_source,
            held,
            arms.shipped
        ),
        arms.text()
    )
}

/// The first clause of [`rpm_lowered_text`].
fn rpm_lowered_head(
    rpm_from: f64,
    rpm_to: f64,
    feed_ceiling_mm_min: f64,
    rpm_floor: f64,
    floor_source: RpmFloorSource,
    held: bool,
    shipped: CapArm,
) -> String {
    if shipped == CapArm::SameRpmSmallerChip {
        return format!(
            "RPM held at {rpm_from:.0}: the chip is thinner at the {feed_ceiling_mm_min:.0} \
             mm/min feed limit of the machine"
        );
    }
    let head = format!(
        "RPM lowered from {rpm_from:.0} to {rpm_to:.0} to keep the chip at the \
         {feed_ceiling_mm_min:.0} mm/min feed limit of the machine"
    );
    if held {
        head
    } else {
        let floor = match floor_source {
            RpmFloorSource::RowRpmMin => "the lowest RPM of the vendor data",
            RpmFloorSource::MachineMinimum => "the lowest RPM of the machine",
        };
        format!(
            "{head}; the RPM stops at {rpm_to:.0} ({floor} is {rpm_floor:.0}), so the chip is \
             thinner at the feed limit"
        )
    }
}

/// What set the lowest RPM that the Step 7 descent may reach (ruling R4
/// Q10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpmFloorSource {
    /// The matched vendor row's published `rpm_min`.
    RowRpmMin,
    /// The machine's minimum spindle speed (the row has no `rpm_min`).
    MachineMinimum,
}

/// Q1 (feeds contract P8, operator ruling 2026-11-03): the two arms that
/// Step 7 compares when the cutting-feed cap binds. Both arms run at the
/// capped feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapArm {
    /// Arm L: the RPM comes down so the chip holds (ruling R4 Q10). The
    /// RPM is the highest the spindle runs at or below `cap / chip per
    /// rev`, not under the floor.
    LowerRpmSameChip,
    /// Arm S: the RPM holds and the chip is thinner at the cap.
    SameRpmSmallerChip,
}

impl CapArm {
    /// The arm in words, for the card.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LowerRpmSameChip => "lower rpm, same chip",
            Self::SameRpmSmallerChip => "same rpm, smaller chip",
        }
    }

    /// The K2 law that the shipped RPM and feed name.
    #[must_use]
    pub const fn law(self) -> &'static str {
        match self {
            Self::LowerRpmSameChip => "Q1 cap arm: lower rpm, same chip",
            Self::SameRpmSmallerChip => "Q1 cap arm: same rpm, smaller chip",
        }
    }
}

/// Q1: one arm at the cap and its power. `demand_kw` is
/// [`cap_arm_demand_kw`]: the worst case of the pre-sim power set (K7) at
/// the arm's RPM and feed; `available_kw` is `machine.power_at_rpm(rpm)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmPower {
    /// The arm.
    pub arm: CapArm,
    /// The RPM of the arm.
    pub rpm: f64,
    /// The feed of the arm (mm/min): the cap, or under it when arm L
    /// cannot hold the chip.
    pub feed: f64,
    /// The power the cut needs (kW) and the case that sets it. `None` when
    /// the material has no force line: the power is not modelled, and the
    /// arm counts as inside.
    pub demand: Option<(engagement::PowerCase, f64)>,
    /// The power the spindle has at `rpm` (kW).
    pub available_kw: f64,
}

impl ArmPower {
    /// The demand in kW, when it is modelled.
    #[must_use]
    pub fn demand_kw(&self) -> Option<f64> {
        self.demand.map(|(_, kw)| kw)
    }

    /// True when the arm is inside the power curve. A demand that is not
    /// modelled is inside (no limit is known).
    #[must_use]
    pub fn inside(&self) -> bool {
        match self.demand_kw() {
            None => true,
            Some(kw) => self.available_kw > 0.0 && kw <= self.available_kw,
        }
    }

    /// How far the demand is over the available power (kW); 0 inside.
    #[must_use]
    pub fn exceedance_kw(&self) -> f64 {
        self.demand_kw()
            .map_or(0.0, |kw| (kw - self.available_kw.max(0.0)).max(0.0))
    }
}

/// Q1: why Step 7 shipped its arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapArmReason {
    /// Only the shipped arm is inside the power curve.
    OnlyInside,
    /// Both arms are inside: arm L ships (lead decision Q1a, R4 Q10 holds;
    /// at the same feed L keeps the larger chip).
    BothInside,
    /// Neither arm is inside: the arm with the smaller exceedance ships,
    /// and `FeedsWarning::PowerLimited` names the power limit.
    NeitherInside,
}

/// Q1: the two arms Step 7 compared and the one it shipped. The card shows
/// both demands and both available powers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapArms {
    /// Arm L.
    pub lower: ArmPower,
    /// Arm S.
    pub same: ArmPower,
    /// The arm that ships.
    pub shipped: CapArm,
    /// Why it ships.
    pub reason: CapArmReason,
}

impl CapArms {
    /// The shipped arm and its power.
    #[must_use]
    pub const fn shipped_power(&self) -> &ArmPower {
        match self.shipped {
            CapArm::LowerRpmSameChip => &self.lower,
            CapArm::SameRpmSmallerChip => &self.same,
        }
    }

    /// The card text: the arm that ships and the power of both arms, for
    /// example "arm: lower rpm, same chip (both inside the power curve);
    /// L 0.512 of 1.014 kW at 16219 rpm, S 0.498 of 1.125 kW at 18000 rpm".
    #[must_use]
    pub fn text(&self) -> String {
        let why = match self.reason {
            CapArmReason::OnlyInside => "the only arm inside the power curve",
            CapArmReason::BothInside => "both inside the power curve",
            CapArmReason::NeitherInside => "neither inside the power curve; the smaller exceedance",
        };
        let arm = |a: &ArmPower| match a.demand_kw() {
            Some(kw) => format!("{kw:.3} of {:.3} kW at {:.0} rpm", a.available_kw, a.rpm),
            None => format!("power not modelled at {:.0} rpm", a.rpm),
        };
        format!(
            "arm: {} ({why}); L {}, S {}",
            self.shipped.label(),
            arm(&self.lower),
            arm(&self.same)
        )
    }
}

/// Q1: pick the arm that ships. Exactly one inside: that one. Both
/// inside: arm L (lead decision Q1a). Neither inside: the smaller
/// exceedance, arm L on a tie.
#[must_use]
pub fn pick_cap_arm(lower: ArmPower, same: ArmPower) -> (ArmPower, CapArmReason) {
    match (lower.inside(), same.inside()) {
        (true, true) => (lower, CapArmReason::BothInside),
        (true, false) => (lower, CapArmReason::OnlyInside),
        (false, true) => (same, CapArmReason::OnlyInside),
        (false, false) => {
            if same.exceedance_kw() < lower.exceedance_kw() {
                (same, CapArmReason::NeitherInside)
            } else {
                (lower, CapArmReason::NeitherInside)
            }
        }
    }
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
    /// Ruling R4 Q10 (2026-09-24): the machine cutting-feed ceiling bound,
    /// and the RPM came down to hold the chipload instead of the chip
    /// thinning at the same RPM. See Step 7 in [`calculate`] and
    /// [`FeedsWarning::RpmLoweredForFeedCeiling`].
    FeedCeiling,
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
        // The observed axial chip thinning is NOT included: the feed does
        // not take it. The radial lift is (P11b, L1).
        //
        // Ruling R4 (2026-09-24): the machine safety factor is gone, and the
        // long-tool share `ld_overhang` is a load target, not a feed factor
        // (Q7), so neither is composed here. P11b (L6): the vendor depth
        // table is a limit of the envelope, not a feed factor.
        self.radial_chip_thinning * self.power_limit * self.feed_clamp
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
    /// Ruling R4 Q10 (2026-09-24): the machine cutting-feed ceiling bound,
    /// so Step 7 lowered the RPM to hold the chipload (the feed moves down
    /// with the RPM, the advance per tooth stays). The descent stops at the
    /// matched row's `rpm_min`, or at the machine's minimum RPM when the row
    /// has none; a VFD's `power_at_rpm` at the lower speed also bounds it.
    ///
    /// The card line: "RPM lowered 18 000 -> 15 750 to hold the chip at the
    /// 4000 mm/min feed ceiling". When `held` is false it adds the stop and
    /// the thinner chip.
    RpmLoweredForFeedCeiling {
        /// The RPM before Step 7.
        rpm_from: f64,
        /// The RPM that ships.
        rpm_to: f64,
        /// The machine cutting-feed ceiling (mm/min).
        feed_ceiling_mm_min: f64,
        /// The lowest RPM the descent may reach: the row's `rpm_min`, or
        /// the machine minimum.
        rpm_floor: f64,
        /// What set `rpm_floor`.
        floor_source: RpmFloorSource,
        /// `true` when the chipload is held in full; `false` when the floor
        /// stopped the descent, or arm S shipped (Q1), and the feed sits
        /// on the ceiling with a thinner chip.
        held: bool,
        /// Q1 (P8): the two arms and their power; `arms.shipped` is the arm
        /// that ships. With arm S, `rpm_to` equals `rpm_from`.
        arms: CapArms,
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
    /// The depth is above the flute guard (Step 4). P11b: a stated depth
    /// ships (`shipped_mm == requested_mm`); a calculator depth is lowered
    /// to the guard (`shipped_mm == guard_mm`).
    DocExceedsFlute {
        requested_mm: f64,
        guard_mm: f64,
        shipped_mm: f64,
    },
    /// The cut is a slot and the depth is above the slot cap (Step 4b).
    /// P11b: a stated depth ships; a calculator depth is lowered to the cap.
    SlottingDetected {
        requested_mm: f64,
        cap_mm: f64,
        shipped_mm: f64,
    },
    ScallopInvalid {
        target: f64,
        max_possible: f64,
    },
    /// The commanded advance per tooth is below the rubbing floor.
    ///
    /// Below the floor the edge rubs instead of cutting. This heats the
    /// edge and can burn the work. The usual causes are a hard species that
    /// scales a vendor row down, a small tool, and the feed de-rates.
    ///
    /// The engine does NOT raise the feed to the floor (ruling R4,
    /// 2026-09-23, WP2a). The warning is the whole response. The operator
    /// has two levers: raise the feed, or lower the RPM. The floor value is
    /// [`rubbing_floor`]: the matched band minimum when it is below the
    /// repo constant [`RUBBING_FLOOR_MM_TOOTH`], otherwise the constant
    /// (repo rule, unsourced). `source` names the bound (ruling R4 Q9).
    ChiploadBelowRubbingFloor {
        /// The commanded advance per tooth (mm/tooth). The engine ships it
        /// unchanged.
        commanded: f64,
        /// The floor that the test used (mm/tooth). See [`rubbing_floor`].
        floor: f64,
        /// The bound that set `floor`.
        source: RubbingFloorSource,
        /// The derated minimum of the band that the floor read, when a band
        /// with a minimum exists.
        band_min: Option<f64>,
        /// The derated maximum of the band that the floor read, when a band
        /// exists.
        band_max: Option<f64>,
    },
    /// The long-tool share of the load target (ruling R4 WP1, 2026-09-23;
    /// moved into the dial by ruling R4 Q7, 2026-09-24).
    ///
    /// A stickout above 4 x D gives the share 0.88. A stickout above 6 x D
    /// gives 0.75. The rule is a repo rule and it is unsourced. Since Q7 the
    /// feed does not take the share, and the chipload stays. The warning is
    /// the visible record.
    LongToolDerate {
        /// The tool stickout from the holder (mm).
        stickout_mm: f64,
        /// The tool diameter that the ratio divides by (mm).
        diameter_mm: f64,
        /// `stickout_mm / diameter_mm`.
        ratio: f64,
        /// The share of the load target (0.88 or 0.75). Not a feed factor.
        factor: f64,
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
    /// (so the band midpoint has nothing to aim
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
        /// [`RUBBING_FLOOR_MM_TOOTH`] still applies. The row can print a
        /// band or one value (a point, A2; see [`rubbing_floor_at`]).
        ///
        /// This is the one thing the recommendation now *does* take from a
        /// chipload-bearing row. The target still comes from the formula and
        /// `chipload_bounds` is still `None`, so this field is not a band —
        /// it is provenance for the floor test, and naming it is what keeps
        /// "the floor" and "the gate's envelope" from being two different
        /// rows the operator cannot tell apart.
        floor_band_from: Option<String>,
    },
    /// **Checkpoint K (a4), 2026-08-13 — the vendor-LUT routing refused
    /// this operation × cutter pairing, so there is no vendor row at all
    /// and the recommendation is entirely formula-derived.**
    ///
    /// Today this is exactly one case: a `ProjectCurve` on a facing
    /// cutter. `ProjectCurve` is not a vendor family; it is geometrically
    /// a 3D contour trace, and [`vendor_normalize::lut_query_for`] routes
    /// it to `Parallel` for ball, tapered-ball and bull-nose cutters (the
    /// bull nose since A3 step 4, through the G3 family rule), to
    /// `Contour` for flat cutters, to `Trace` for a V-bit (operator ruling
    /// 2026-09-25: the tool follows the curve at a set depth, the same
    /// engagement a v-carve or trace pass has), and refuses only for a
    /// facing bit, because the LUT has no rows there.
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
    /// Feeds contract P13 item 4 (law B): a flat end mill on an op that
    /// reads the (Pocket, Roughing) rows of its tool
    /// (`LutRoute::FlatAsRoughPocket`) found no printed row, so the chip is
    /// the machine chip formula (`machine::ChipLoadFormula`, Unsourced).
    /// D7: a warning, never a refusal (Q6). The shipped formula stays only
    /// on this path for those ops.
    FormulaChipNoPrintedRow {
        /// The formula advance per tooth the cell ships (mm/tooth).
        formula_chipload_mm: f64,
    },
    /// Drill-cycle feed clamped into the material plunge-feed envelope
    /// (`Material::drill_plunge_feed_envelope_per_mm` × diameter,
    /// mm/min). Below the envelope the drill rubs and burns; above it
    /// the bit risks breakage. Mirrors the drill plunge-feed gate
    /// (`tool_load/drill_gates.rs`) so Suggest and the verdict share
    /// one envelope source. Since ruling B5 the wood ceilings are the
    /// largest Amana Ramp Down figures per mm (580 wood and plywood,
    /// 720 MDF), so no G6-claimed cell reaches them; the floor and a
    /// formula preview still can.
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
        /// Why. A `Cow` since 2026-09-24: the size refusal names the two
        /// diameters (`support::micro_extrapolation_refusal`).
        reason: std::borrow::Cow<'static, str>,
    },
    /// A tool dimension is outside its domain
    /// ([`crate::compute::tool_config::ToolConfig::validate`], F029). Suggest
    /// writes no recipe for a tool that no cutter can be built from.
    ToolOutOfDomain(crate::tool::ToolGeometryError),
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
            FeedsError::ToolOutOfDomain(e) => write!(f, "Suggest refuses this tool: {e}."),
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
/// extreme diameter shrinkage — raises a
/// `FeedsWarning::ChiploadBelowRubbingFloor` warning. The engine does not
/// raise the feed (ruling R4 WP2a, 2026-09-23). The value is a repo rule
/// with no primary source.
///
/// **This is a *global* threshold, not the value the warning tests.**
/// It carries no diameter and no material, while every other chipload
/// bound in the crate is a scaled vendor band. The warning tests
/// [`effective_rubbing_floor`], which subordinates this constant to the
/// matched band's minimum, or to its maximum when the band has no minimum
/// (ruling R4 Q9).
pub const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

/// The bound that set the rubbing floor (ruling R4 Q9, 2026-09-24).
///
/// The cards and the diagnostics read this value to state the source status
/// of the floor. A number alone cannot say which bound set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RubbingFloorSource {
    /// The repo constant [`RUBBING_FLOOR_MM_TOOTH`] (repo rule, unsourced).
    /// The band minimum is at or above it, or no band exists.
    RepoConstant,
    /// The published minimum of the matched vendor band. It is below the
    /// repo constant, and a published value outranks the unsourced constant
    /// (EVIDENCE 4.1-26).
    BandMinimum,
    /// The maximum of a band that publishes no minimum (a band with
    /// `min_mm_per_tooth == 0.0`, for example the gate's band on a
    /// maximum-only row). The maximum is below the repo constant.
    BandMaximum,
    /// The one printed value of a point row (A2, point mode). The row
    /// prints no band, and the point is below the repo constant.
    PrintedPoint,
}

impl RubbingFloorSource {
    /// The source status of the floor, in words. The diagnostics adapter,
    /// the Suggest rationale and the Feeds card print this one text.
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::RepoConstant => "a fixed limit with no published source".to_owned(),
            Self::BandMinimum => format!(
                "the lowest vendor value, below the {RUBBING_FLOOR_MM_TOOTH:.3} mm fixed limit"
            ),
            Self::BandMaximum => format!(
                "the highest vendor value; the vendor gives no lowest value, and this one is \
                 below the {RUBBING_FLOOR_MM_TOOTH:.3} mm fixed limit"
            ),
            Self::PrintedPoint => format!(
                "the one vendor value (no range), below the {RUBBING_FLOOR_MM_TOOTH:.3} mm \
                 fixed limit"
            ),
        }
    }
}

/// The rubbing floor at a band, and the bound that set it.
///
/// The rule (ruling R4 Q9, 2026-09-24), in this order:
///
/// 1. The band has a minimum (`min > 0`): `min(0.025, band min)`.
/// 2. The band has no minimum and has a maximum: `min(0.025, band max)`.
/// 3. No band: the constant 0.025.
///
/// Arithmetic: a band 0.0036–0.0072 mm/tooth gives the floor 0.0036
/// (`BandMinimum`). A band 0.034–0.059 gives 0.025 (`RepoConstant`). A band
/// 0.0–0.012 gives 0.012 (`BandMaximum`).
#[must_use]
pub fn rubbing_floor(band: Option<ChiploadBounds>) -> (f64, RubbingFloorSource) {
    let (bound, source) = match band {
        Some(b) if b.min_mm_per_tooth > 0.0 => {
            (b.min_mm_per_tooth, RubbingFloorSource::BandMinimum)
        }
        Some(b) if b.max_mm_per_tooth > 0.0 => {
            (b.max_mm_per_tooth, RubbingFloorSource::BandMaximum)
        }
        _ => return (RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource::RepoConstant),
    };
    if bound < RUBBING_FLOOR_MM_TOOTH {
        (bound, source)
    } else {
        (RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource::RepoConstant)
    }
}

/// The rubbing floor at a band or at a printed point (A2, point mode).
///
/// A band gives [`rubbing_floor`] unchanged. With no band, a point `v`
/// gives `min(0.025, v)`: the point with [`RubbingFloorSource::PrintedPoint`]
/// when `v` is below the constant, else the constant. With neither, the
/// constant applies.
///
/// A row that prints one value has no band (`chipload_bounds` is `None`),
/// so before A2 this rule (R4 Q9 rule 2) did not reach it.
#[must_use]
pub fn rubbing_floor_at(
    band: Option<ChiploadBounds>,
    point_mm_per_tooth: Option<f64>,
) -> (f64, RubbingFloorSource) {
    if band.is_some() {
        return rubbing_floor(band);
    }
    match point_mm_per_tooth {
        Some(v) if v.is_finite() && v > 0.0 && v < RUBBING_FLOOR_MM_TOOTH => {
            (v, RubbingFloorSource::PrintedPoint)
        }
        _ => rubbing_floor(None),
    }
}

/// The floor that the Step-9b warning tests. It is the value half of
/// [`rubbing_floor`]; read that function for the rule and the source.
///
/// Until ruling R4 WP2a (2026-09-23) Step 9b also raised the feed to this
/// floor. It now only warns. Until ruling R4 Q9 (2026-09-24) the floor was
/// `min(0.025, band max)`, so a recipe inside a published band below 0.025
/// warned. The published band minimum now outranks the unsourced constant:
/// a recipe inside the vendor band does not warn.
///
/// # Bands that do not exist
///
/// RPM-only vendor rows and the formula fallback publish no chipload
/// window (`chipload_bounds == None`); there is nothing to subordinate
/// to and the global constant applies unmodified. That path is pinned
/// by `tests/_litmatrix_rpm_only_lut_chipload.rs`.
///
/// FEEDS_CENSUS C-12 / T3.3 / T4.2; ruled 2026-08-06; re-ruled R4 Q9.
#[must_use]
pub fn effective_rubbing_floor(band: Option<ChiploadBounds>) -> f64 {
    rubbing_floor(band).0
}

// ---------------------------------------------------------------------
// F201: the formula-path constants of `feeds::calculate` and their source.
// `material::ConstantSource` explains the two kinds. The registry
// `formula_constants` lists every row;
// `tests/every_feeds_formula_constant_states_its_source_f201.rs` pins the
// unsourced count.
// ---------------------------------------------------------------------

/// The RPM when the engaged diameter is zero or less (Step 1).
const FALLBACK_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.calculate.fallback_rpm",
    18_000.0,
    "a guard value for a zero engaged diameter; no record in the repo",
);

const DRILL_TIER_CLAIM: &str =
    "repo rule; the W6 audit (2026-08-04) found that the cited sources do not state the tiers";
const DRILL_TIER_SMALL_MAX_D_MM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.small_max_d_mm",
    6.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_MID_MAX_D_MM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.mid_max_d_mm",
    10.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_SMALL_FLOOR_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.small_floor_rpm",
    8_000.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_SMALL_CEILING_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.small_ceiling_rpm",
    14_000.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_MID_FLOOR_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.mid_floor_rpm",
    6_000.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_MID_CEILING_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.mid_ceiling_rpm",
    10_000.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_LARGE_FLOOR_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.large_floor_rpm",
    4_000.0,
    DRILL_TIER_CLAIM,
);
const DRILL_TIER_LARGE_CEILING_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.drill_rpm_envelope_for_diameter.large_ceiling_rpm",
    8_000.0,
    DRILL_TIER_CLAIM,
);

/// Diameter-tiered RPM envelope for wood-drilling ops: a named repo cap
/// (repo rule, unsourced; CREDITS.md "Drill-subsystem provenance"). The
/// band narrows and drops as the diameter grows, because chip evacuation
/// scales with the chip volume per revolution. The W6 audit (2026-08-04)
/// found that the sources this doc used to cite do not state these tiers.
///
/// On a G6-claimed plunge (ruling B5) this cap replaces the 18 000 RPM
/// that the Amana Spektra chart prints: Step 2c clamps the vendor RPM to
/// it, the chip per tooth holds, and the feed follows. The claim card
/// states it (`extrapolation::drill::DrillClaim::card_text`).
///
/// Tiers (inclusive upper bound):
/// - D ≤ 6 mm:  (8000, 14000).
/// - D ≤ 10 mm: (6000, 10000).
/// - D > 10 mm: (4000, 8000); the 8 kRPM ceiling matches the
///   literature-matrix `flat_12mm_drill_oak_big` cell.
#[must_use]
pub(crate) fn drill_rpm_envelope_for_diameter(d_mm: f64) -> (f64, f64) {
    let (floor, ceiling) = drill_rpm_tier_rows(d_mm);
    (floor.value, ceiling.value)
}

/// The two F201 rows (floor, ceiling) of the drill RPM tier at `d_mm`: the
/// rows that [`drill_rpm_envelope_for_diameter`] reads (P5: the basis of a
/// drill RPM).
fn drill_rpm_tier_rows(d_mm: f64) -> (FormulaConstant, FormulaConstant) {
    if d_mm <= DRILL_TIER_SMALL_MAX_D_MM.value {
        (DRILL_TIER_SMALL_FLOOR_RPM, DRILL_TIER_SMALL_CEILING_RPM)
    } else if d_mm <= DRILL_TIER_MID_MAX_D_MM.value {
        (DRILL_TIER_MID_FLOOR_RPM, DRILL_TIER_MID_CEILING_RPM)
    } else {
        (DRILL_TIER_LARGE_FLOOR_RPM, DRILL_TIER_LARGE_CEILING_RPM)
    }
}

/// The tiers of [`milling_rpm_ceiling_for_diameter`]: (largest diameter in
/// mm, ceiling in RPM). The last row has an infinite diameter.
const MILLING_CEILING_CLAIM: &str =
    "the doc names Onsrud 70/85, Amana Spektra and GWizard bands; no file in the repo prints them";
const MILLING_CEILING_TIERS: [(FormulaConstant, FormulaConstant); 4] = [
    (
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier1_max_d_mm",
            3.0,
            MILLING_CEILING_CLAIM,
        ),
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier1_ceiling_rpm",
            22_000.0,
            MILLING_CEILING_CLAIM,
        ),
    ),
    (
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier2_max_d_mm",
            6.0,
            MILLING_CEILING_CLAIM,
        ),
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier2_ceiling_rpm",
            20_000.0,
            MILLING_CEILING_CLAIM,
        ),
    ),
    (
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier3_max_d_mm",
            8.0,
            MILLING_CEILING_CLAIM,
        ),
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier3_ceiling_rpm",
            18_000.0,
            MILLING_CEILING_CLAIM,
        ),
    ),
    (
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier4_max_d_mm",
            10.0,
            MILLING_CEILING_CLAIM,
        ),
        FormulaConstant::unsourced(
            "feeds.milling_rpm_ceiling_for_diameter.tier4_ceiling_rpm",
            16_000.0,
            MILLING_CEILING_CLAIM,
        ),
    ),
];
/// The ceiling above the last tier of [`MILLING_CEILING_TIERS`].
const MILLING_CEILING_LARGE_RPM: FormulaConstant = FormulaConstant::unsourced(
    "feeds.milling_rpm_ceiling_for_diameter.large_ceiling_rpm",
    14_000.0,
    MILLING_CEILING_CLAIM,
);

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
    milling_rpm_ceiling_row(d_mm).value
}

/// The F201 row of [`milling_rpm_ceiling_for_diameter`] at `d_mm` (P5: the
/// basis of a tier-capped RPM).
fn milling_rpm_ceiling_row(d_mm: f64) -> FormulaConstant {
    MILLING_CEILING_TIERS
        .iter()
        .find(|(max_d, _)| d_mm <= max_d.value)
        .map_or(MILLING_CEILING_LARGE_RPM, |(_, ceiling)| ceiling.clone())
}

/// The F201 row of [`MAX_SPINDLE_SPEEDUP`].
const MAX_SPINDLE_SPEEDUP_ROW: FormulaConstant = FormulaConstant::unsourced(
    "feeds.MAX_SPINDLE_SPEEDUP",
    MAX_SPINDLE_SPEEDUP,
    "the rough magnitude of a chart's rpm_max over rpm_nominal; no record in the repo",
);

/// The basis `b`, or a law over `b` and `cap` when a step moved the value
/// from `before` to `after` (P5: each step writes its basis with its value).
fn moved_basis(b: Basis, before: f64, after: f64, law: &'static str, cap: Basis) -> Basis {
    if (after - before).abs() > 1e-12 {
        Basis::derived(law, vec![b, cap])
    } else {
        b
    }
}

/// The basis of a matched row's chip: the printed row, the band midpoint of
/// the printed row, or the claim (G1, G2, G3, G6) that scaled it.
fn chip_basis_of_row(row: &vendor_lookup::LookupResult) -> Basis {
    use sourced::{ClaimKind, ClaimRef, RowRef};
    let row_ref = RowRef(row.observation_id.clone());
    let claim = |kind: ClaimKind, rule: &str| {
        Basis::Claim(ClaimRef {
            kind,
            row: row_ref.clone(),
            rule: rule.to_owned(),
        })
    };
    if row.drill_basis.claim().is_some() {
        return claim(ClaimKind::DrillG6, "g6_drill");
    }
    if row.size_basis.claim().is_some() {
        return claim(ClaimKind::SizeG1, "g1_size");
    }
    if (row.chipload_hardness_scale - 1.0).abs() > 1e-12 {
        return claim(ClaimKind::HardnessG2, "g2_hardness");
    }
    if row.family_basis.claim().is_some() {
        return claim(ClaimKind::FamilyG3, "g3_family");
    }
    match row.printed_chipload() {
        vendor_lookup::PrintedChipload::Band { .. } => {
            Basis::derived("band midpoint", vec![Basis::Printed(row_ref)])
        }
        vendor_lookup::PrintedChipload::Point { .. }
        | vendor_lookup::PrintedChipload::Unpublished => Basis::Printed(row_ref),
    }
}

/// Assemble the Suggest path's inputs to the canonical two-term power
/// model (`tool_load::power`) at the width `ae`, through the one engagement
/// (K7, `feeds::engagement::Engagement::from_cut`), so Step 6's clamp and
/// the published `power_kw` are built from one expression.
///
/// `effective_d` must be the CHIP-THINNING diameter — the circle that
/// actually touches material at this DOC — because it sets both the
/// immersion angle ψ and the cutting velocity `Vc = π·D·n`. That is the
/// same binding `RadialThinning::lift` reads, not the LUT-row
/// diameter this function's callers shadowed earlier.
fn power_model_terms(
    input: &FeedsInput,
    line: crate::material::force_line::ForceLine,
    ap: f64,
    ae: f64,
    effective_d: f64,
    rpm: f64,
) -> crate::tool_load::power::PowerTerms {
    crate::tool_load::power::PowerTerms::of(
        engagement::Engagement::from_cut(
            envelope::CutPoint {
                ap_mm: ap,
                ae_mm: ae,
            },
            input.tool_geometry,
            effective_d,
            None,
        )
        .power_inputs(line, rpm, f64::from(input.flute_count)),
    )
}

/// K7 change 3: the power (kW) at `feed` of the worst case of the pre-sim
/// set (`FeedsInput::power_cases`) at the nominal width `ae`, with the case
/// and its width. Both terms grow with the width, so the worst case is the
/// widest one at every feed and RPM.
fn worst_case_power(
    input: &FeedsInput,
    line: crate::material::force_line::ForceLine,
    ap: f64,
    ae: f64,
    effective_d: f64,
    rpm: f64,
    feed: f64,
) -> (engagement::PowerCase, f64, f64) {
    input.power_cases.worst(ae, input.tool_diameter, |w| {
        power_model_terms(input, line, ap, w, effective_d, rpm).kw_at_feed(feed)
    })
}

/// Q1 (feeds contract P8): THE demand of a cap arm. The worst case of the
/// K7 pre-sim set of the `CutSpec` (`FeedsInput::power_cases` at the stated
/// `ap` and `ae`) at the arm's `rpm` and `feed`, through `Engagement`, with
/// its case. `None` when the material has no force line.
#[must_use]
pub(crate) fn cap_arm_demand_kw(
    input: &FeedsInput,
    material: &Material,
    ap: f64,
    ae: f64,
    effective_d: f64,
    rpm: f64,
    feed: f64,
) -> Option<(engagement::PowerCase, f64)> {
    let line = material.force_line().ok()?;
    let (case, kw, _) = worst_case_power(input, line, ap, ae, effective_d, rpm, feed);
    Some((case, kw))
}

/// Step 4: the depth cap as a share of the flute length.
const FLUTE_GUARD_FACTOR: FormulaConstant = FormulaConstant::unsourced(
    "feeds.calculate.flute_guard_factor",
    0.8,
    "no comment and no record in the repo",
);
/// Step 4: the depth cap in diameters when the flute length is unknown.
const FLUTE_GUARD_NO_LENGTH_D: FormulaConstant = FormulaConstant::unsourced(
    "feeds.calculate.flute_guard_no_length_d",
    2.0,
    "no comment and no record in the repo",
);
/// Step 4: the flute guard depth and its basis (P5: the calculator and the
/// envelope read one rule). `length_kind` is the kind of the flute length
/// fact (P13a): an `Unknown` length plans at its legacy row, and the basis
/// names that row.
pub(crate) fn flute_guard_of(
    flute_length_mm: f64,
    length_kind: crate::compute::tool_config::FactKind,
    d: f64,
) -> (f64, Basis) {
    if flute_length_mm > 0.0 {
        (
            flute_length_mm * FLUTE_GUARD_FACTOR.value,
            Basis::derived(
                "0.8 x flute",
                vec![
                    Basis::Rule(FLUTE_GUARD_FACTOR),
                    Basis::tool_fact(sourced::ToolFactName::CuttingLength, length_kind),
                ],
            ),
        )
    } else {
        (
            d * FLUTE_GUARD_NO_LENGTH_D.value,
            Basis::derived(
                "2 x D (no flute length)",
                vec![Basis::Rule(FLUTE_GUARD_NO_LENGTH_D), Basis::tool_diameter()],
            ),
        )
    }
}

/// Step 4b: the slot rule (threshold in diameters, depth cap in
/// diameters) and its basis (P5: the calculator and the envelope read one
/// rule). P11c (D13 item 3): the depth cap is the machine's
/// `RigidityProfile::slot_doc_factor`; the threshold stays a
/// machine-neutral geometry rule.
pub(crate) fn slot_rule(machine: &MachineProfile) -> (f64, f64, Basis) {
    (
        SLOTTING_THRESHOLD.value,
        machine.rigidity.slot_doc_factor,
        Basis::derived(
            "slot cap",
            vec![Basis::Rule(SLOTTING_THRESHOLD), rigidity_basis(machine)],
        ),
    )
}

/// P11c (D13 item 6): the basis of a value that a rigidity factor of
/// `machine` sets. The law names the machine class and how the values are
/// known (`RigidityBasis::text`, for example "hobby grade (Shapeoko port),
/// Unsourced"), so the card, MCP, the CLI and the K8 lineage print the
/// class beside the value.
#[must_use]
pub fn rigidity_basis(machine: &MachineProfile) -> Basis {
    Basis::derived(
        machine.rigidity_basis().text(),
        vec![Basis::Machine(sourced::MachineField::Rigidity)],
    )
}

/// Step 4b: the radial width, in diameters, above which the cut is a slot.
const SLOTTING_THRESHOLD: FormulaConstant = FormulaConstant::unsourced(
    "feeds.calculate.slotting_threshold_d",
    0.85,
    "no comment and no record in the repo",
);

/// Main calculation entry point.
/// Smallest axial depth the power ladder may propose (mm).
///
/// Without a floor a solver that cannot reach the budget walks the geometry
/// to zero and ships a recipe that removes no material. (The deleted
/// deflection back-off had a 0.5 mm floor for the same reason.) When the floor binds, the cut stays over budget and
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

/// The smallest axial depth the calculator writes (mm). The envelope draws
/// the same floor (`LimitKind::EngagementFloor`).
pub(crate) const MIN_AP_MM: f64 = 0.05;
/// The smallest radial width the calculator writes (mm). The envelope draws
/// the same floor (`LimitKind::EngagementFloor`).
pub(crate) const MIN_AE_MM: f64 = 0.02;
/// F201 (feeds contract P13a, review H3b): the row of [`MIN_AP_MM`]. A
/// depth that the floor raised, and the envelope depth floor, read it.
pub(crate) const MIN_AP_MM_ROW: FormulaConstant = FormulaConstant::unsourced(
    "feeds.MIN_AP_MM",
    MIN_AP_MM,
    "the smallest depth the calculator writes; no record in the repo",
);
/// F201 (feeds contract P13a, review H3b): the row of [`MIN_AE_MM`]. A
/// width that the floor raised, and the envelope width floor, read it.
pub(crate) const MIN_AE_MM_ROW: FormulaConstant = FormulaConstant::unsourced(
    "feeds.MIN_AE_MM",
    MIN_AE_MM,
    "the smallest width the calculator writes; no record in the repo",
);

/// L/D above which the long-tool share is [`LD_SEVERE_SHARE`].
const LD_SEVERE_THRESHOLD: f64 = 6.0;
/// L/D above which the long-tool share is [`LD_MODERATE_SHARE`].
const LD_MODERATE_THRESHOLD: f64 = 4.0;
/// The load-target share above 6 x D stickout (repo rule, unsourced).
const LD_SEVERE_SHARE: f64 = 0.75;
/// The load-target share above 4 x D stickout (repo rule, unsourced).
const LD_MODERATE_SHARE: f64 = 0.88;

/// The long-tool share of the load target for a stickout and a diameter
/// (ruling R4 Q7, 2026-09-24): 0.75 above 6 x D, 0.88 above 4 x D, else 1.0.
/// A repo rule with no source. `feeds::calculate` Step 5b reports it. It
/// never multiplies the feed.
#[must_use]
pub fn long_tool_load_share(stickout_mm: f64, diameter_mm: f64) -> f64 {
    if !(stickout_mm.is_finite() && diameter_mm.is_finite()) || diameter_mm <= 0.0 {
        return 1.0;
    }
    let ratio = stickout_mm / diameter_mm;
    if ratio > LD_SEVERE_THRESHOLD {
        LD_SEVERE_SHARE
    } else if ratio > LD_MODERATE_THRESHOLD {
        LD_MODERATE_SHARE
    } else {
        1.0
    }
}

/// The feeds calculator: one point (RPM, chip, feed, plunge, depth, width,
/// power) and the basis of each value. The steps are in `calculator`
/// (feeds contract P13a).
pub fn calculate(input: &FeedsInput) -> FeedsResult {
    calculate_with(input, DepthClamps::Apply, StepRecording::Record)
}

/// The source of every factor in [`operation_default_profile`] (F201).
/// [`formula_constants`] lists one row per factor; it reads each value from
/// the function, so the rows cannot drift from the table.
const OPERATION_DEFAULT_PROFILE_SOURCE: ConstantSource = ConstantSource::Unsourced {
    claim: "DOC/WOC matrix of reference/shapeoko_feeds_and_speeds calcs.rs (not in the repo)",
};

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
        // Pocket: moderate. P13c (ruling O3): the roughing width is the
        // corner-coverage bound, derived (`force_budget::corner_coverage_width_factor`),
        // not the typed 0.35 D (deleted). The calculator reads it on an
        // unstated width; the envelope proposal (the removal-rate pick)
        // sets the width the add door writes.
        (OperationFamily::Pocket, PassRole::Roughing) => DefaultProfile {
            ap_factor: 0.70,
            ae_factor: force_budget::corner_coverage_width_factor(),
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

/// Every constant on the feeds formula path, with its source (F201).
///
/// The formula path is `feeds::calculate` and the `Material` methods that it
/// reads. The rows of [`operation_default_profile`] and of the default
/// [`crate::machine::ChipLoadFormula`] read their values from that code. Every
/// other row is the constant that the code reads. Thus no row can drift from
/// the number that the formula uses.
///
/// Out of the list: design tolerances and solver limits with a stated reason
/// (`SPINDLE_CEILING_HEADROOM`, the power-ladder floors, the bisection
/// limits, the chip-thinning clamp), unit conversions, and the per-machine
/// `RigidityProfile` values. P13a (review H3b, L3, L4) added the engagement
/// floor (`MIN_AP_MM`, `MIN_AE_MM`), the cutter helix default and the drill
/// dwell: each ships a value, so each has a row.
#[must_use]
pub fn formula_constants() -> Vec<FormulaConstant> {
    let mut rows: Vec<FormulaConstant> = MATERIAL_FORMULA_CONSTANTS.to_vec();
    rows.extend([
        FALLBACK_RPM,
        DRILL_TIER_SMALL_MAX_D_MM,
        DRILL_TIER_MID_MAX_D_MM,
        DRILL_TIER_SMALL_FLOOR_RPM,
        DRILL_TIER_SMALL_CEILING_RPM,
        DRILL_TIER_MID_FLOOR_RPM,
        DRILL_TIER_MID_CEILING_RPM,
        DRILL_TIER_LARGE_FLOOR_RPM,
        DRILL_TIER_LARGE_CEILING_RPM,
    ]);
    for (max_d, ceiling) in MILLING_CEILING_TIERS {
        rows.push(max_d);
        rows.push(ceiling);
    }
    rows.push(MILLING_CEILING_LARGE_RPM);
    // K4 (feeds contract P3, lead decision D5): the plunge feed rule of the
    // dressup entry (`PlungeFeedRule::FirstLinearFeedFraction`).
    rows.extend([
        crate::compute::catalog::ENTRY_PLUNGE_FIRST_FEED_FRACTION,
        crate::compute::catalog::ENTRY_PLUNGE_FALLBACK_MM_MIN,
    ]);
    // K3 (feeds contract P6, decision E4): the legacy loader default of
    // each tool fact. An `Unknown` fact plans at it (`tool.fact_unknown`).
    rows.extend(crate::compute::tool_config::LEGACY_DEFAULT_ROWS);
    // P10 (ruling D8): the geometry rule rows of the add door.
    rows.extend(crate::compute::geometry_default::formula_constants());
    rows.extend([
        MAX_SPINDLE_SPEEDUP_ROW,
        // P5 (K2b, card rows 25 and 27): the pass 3 move budget and the
        // pass 6 deflection target and floor, the bases of the `Runtime`
        // and `Deflection` limits of the envelope.
        suggest::STEPOVER_BACKOFF_TARGET_MOVES_ROW,
        suggest::STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW,
        suggest::DEFLECTION_BACKOFF_TARGET_ROW,
        FormulaConstant::unsourced(
            "feeds.RUBBING_FLOOR_MM_TOOTH",
            RUBBING_FLOOR_MM_TOOTH,
            "repo rule with no primary source (the doc says so)",
        ),
        FormulaConstant::unsourced(
            "feeds.LD_SEVERE_THRESHOLD",
            LD_SEVERE_THRESHOLD,
            "repo rule, unsourced (ruling R4 Q7)",
        ),
        FormulaConstant::unsourced(
            "feeds.LD_MODERATE_THRESHOLD",
            LD_MODERATE_THRESHOLD,
            "repo rule, unsourced (ruling R4 Q7)",
        ),
        FormulaConstant::unsourced(
            "feeds.LD_SEVERE_SHARE",
            LD_SEVERE_SHARE,
            "repo rule, unsourced (ruling R4 Q7)",
        ),
        FormulaConstant::unsourced(
            "feeds.LD_MODERATE_SHARE",
            LD_MODERATE_SHARE,
            "repo rule, unsourced (ruling R4 Q7)",
        ),
        FLUTE_GUARD_FACTOR,
        FLUTE_GUARD_NO_LENGTH_D,
        SLOTTING_THRESHOLD,
        ADAPTIVE_FLAT_AP,
        ADAPTIVE_FLAT_AE,
        ADAPTIVE_BALL_AP,
        ADAPTIVE_BALL_AE,
        ADAPTIVE_TAPERED_AP,
        ADAPTIVE_TAPERED_AE,
        ADAPTIVE_SEMI_AP_SCALE,
        ADAPTIVE_SEMI_AE_SCALE,
        ADAPTIVE_MULTI_FLUTE_AE_SCALE,
        ADAPTIVE_HARD_THRESHOLD,
        ADAPTIVE_HARD_AP_SCALE,
        ADAPTIVE_HARD_AE_SCALE,
        ADAPTIVE_MEDIUM_THRESHOLD,
        ADAPTIVE_MEDIUM_AP_SCALE,
        ADAPTIVE_MEDIUM_AE_SCALE,
        ADAPTIVE_SOFT_THRESHOLD,
        ADAPTIVE_SOFT_AP_SCALE,
        ADAPTIVE_SOFT_AE_SCALE,
        ADAPTIVE_RIGIDITY_DOC_DIVISOR,
        FINISH_BALL_AP,
        FINISH_BALL_AE,
        SEMI_FINISH_BALL_AP,
        SEMI_FINISH_BALL_AE,
        FINISH_TAPERED_AP,
        FINISH_TAPERED_AE,
        SEMI_FINISH_TAPERED_AP,
        SEMI_FINISH_TAPERED_AE,
        // P13a (review H3b, L3, L4).
        MIN_AP_MM_ROW,
        MIN_AE_MM_ROW,
        FormulaConstant::unsourced(
            "tool.DEFAULT_HELIX_DEG",
            crate::tool::DEFAULT_HELIX_DEG,
            "the helix of a cutter that states none; typed in five cutter constructors \
             before P13a, no record in the repo",
        ),
        FormulaConstant::unsourced(
            "compute.operation_configs.DRILL_DWELL_S",
            crate::compute::operation_configs::DRILL_DWELL_S,
            "the dwell that the Drill and the Pin Drill shipped before decision D2 \
             (feeds contract P0); no record in the repo",
        ),
    ]);
    // P13c (O1, O3): the hobby force budget (Cited, the chart row that
    // sets it) and the corner-coverage width factor (Cited, derived).
    rows.push(force_budget::hobby_budget_row());
    rows.push(force_budget::corner_coverage_row(
        force_budget::CORNER_COVERAGE_ROW_NAME,
    ));
    let families = crate::all_variants!(OperationFamily:
        Adaptive, Pocket, Contour, Parallel, Scallop, Trace, Face, Drill,
    );
    let roles = crate::all_variants!(PassRole: Roughing, SemiFinish, Finish);
    for family in families {
        // The Drill row is 0 / 0 for type completeness; the drill RPM
        // tiers and the drill feed envelope set the drill operating point.
        if family == OperationFamily::Drill {
            continue;
        }
        for role in roles {
            let profile = operation_default_profile(family, role);
            for (axis, value) in [("ap", profile.ap_factor), ("ae", profile.ae_factor)] {
                rows.push(profile_row(family, role, axis, value));
            }
        }
    }
    // `machine/` owns these defaults; `calculate` reads them through
    // `machine.chip_load` (Step 2).
    let chip_load = crate::machine::ChipLoadFormula::default();
    let chip_load_claim = ConstantSource::Unsourced {
        claim: "\"Soft wood baseline from Shapeoko empirical data\" (reference/, not in the repo)",
    };
    for (key, value) in [("k0", chip_load.k0), ("p", chip_load.p), ("q", chip_load.q)] {
        rows.push(FormulaConstant {
            name: std::borrow::Cow::Owned(format!("machine.ChipLoadFormula.default.{key}")),
            value,
            source: chip_load_claim,
        });
    }
    rows
}

// `default_engagement`: the adaptive matrix per tool shape, its derates, and
// the ball and tapered-ball finish overrides (factors of the diameter).
const ADAPTIVE_CLAIM: &str =
    "adaptive matrix of reference/shapeoko_feeds_and_speeds calcs.rs (not in the repo)";
const FINISH_OVERRIDE_CLAIM: &str =
    "finish override for a ball or a tapered ball; no record in the repo";
const ADAPTIVE_FLAT_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.flat_bull_ap",
    1.20,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_FLAT_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.flat_bull_ae",
    0.14,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_BALL_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.ball_ap",
    0.80,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_BALL_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.ball_ae",
    0.10,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_TAPERED_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.tapered_ball_ap",
    0.70,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_TAPERED_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.tapered_ball_ae",
    0.08,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_SEMI_AP_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.semi_finish_ap_scale",
    0.75,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_SEMI_AE_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.semi_finish_ae_scale",
    0.85,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_MULTI_FLUTE_AE_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.three_or_more_flutes_ae_scale",
    0.85,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_HARD_THRESHOLD: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.hard_feed_scale_threshold",
    1.40,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_HARD_AP_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.hard_ap_scale",
    0.80,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_HARD_AE_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.hard_ae_scale",
    0.90,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_MEDIUM_THRESHOLD: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.medium_feed_scale_threshold",
    1.15,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_MEDIUM_AP_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.medium_ap_scale",
    0.90,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_MEDIUM_AE_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.medium_ae_scale",
    0.95,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_SOFT_THRESHOLD: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.soft_feed_scale_threshold",
    0.85,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_SOFT_AP_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.soft_ap_scale",
    1.05,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_SOFT_AE_SCALE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.soft_ae_scale",
    1.05,
    ADAPTIVE_CLAIM,
);
const ADAPTIVE_RIGIDITY_DOC_DIVISOR: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.adaptive.rigidity_doc_divisor",
    1.50,
    ADAPTIVE_CLAIM,
);
const FINISH_BALL_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.finish.ball_ap",
    0.06,
    FINISH_OVERRIDE_CLAIM,
);
const FINISH_BALL_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.finish.ball_ae",
    0.025,
    FINISH_OVERRIDE_CLAIM,
);
const SEMI_FINISH_BALL_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.semi_finish.ball_ap",
    0.10,
    FINISH_OVERRIDE_CLAIM,
);
const SEMI_FINISH_BALL_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.semi_finish.ball_ae",
    0.04,
    FINISH_OVERRIDE_CLAIM,
);
const FINISH_TAPERED_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.finish.tapered_ball_ap",
    0.10,
    FINISH_OVERRIDE_CLAIM,
);
const FINISH_TAPERED_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.finish.tapered_ball_ae",
    0.03,
    FINISH_OVERRIDE_CLAIM,
);
const SEMI_FINISH_TAPERED_AP: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.semi_finish.tapered_ball_ap",
    0.14,
    FINISH_OVERRIDE_CLAIM,
);
const SEMI_FINISH_TAPERED_AE: FormulaConstant = FormulaConstant::unsourced(
    "feeds.default_engagement.semi_finish.tapered_ball_ae",
    0.05,
    FINISH_OVERRIDE_CLAIM,
);

/// The F201 row of one factor of [`operation_default_profile`] (P5: the
/// basis of a family default). The name is the one that
/// [`formula_constants`] lists.
fn profile_row(family: OperationFamily, role: PassRole, axis: &str, value: f64) -> FormulaConstant {
    FormulaConstant {
        name: std::borrow::Cow::Owned(format!(
            "feeds.operation_default_profile.{family:?}.{role:?}.{axis}_factor"
        )),
        value,
        source: profile_row_source(family, role, axis),
    }
}

/// The source of one profile row: P13c (O3) makes the (Pocket, Roughing)
/// width the derived corner-coverage bound (Cited, `force_budget::corner_coverage_row`);
/// every other row is the old Unsourced port.
fn profile_row_source(family: OperationFamily, role: PassRole, axis: &str) -> ConstantSource {
    if family == OperationFamily::Pocket && role == PassRole::Roughing && axis == "ae" {
        force_budget::corner_coverage_row(force_budget::CORNER_COVERAGE_ROW_NAME).source
    } else {
        OPERATION_DEFAULT_PROFILE_SOURCE
    }
}

/// The axis of a family default row ([`default_profile_row`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAxisName {
    /// `ap_factor`.
    Ap,
    /// `ae_factor`.
    Ae,
}

/// P10 (ruling D8): the F201 row of the family default `k` of `family` and
/// `role` on `axis`: the `operation_default_profile` row that the
/// calculator reads on an unstated axis. The add door's geometry rule
/// (`compute::geometry_default::KRow::FamilyProfile`) reads the same row.
#[must_use]
pub fn default_profile_row(
    family: OperationFamily,
    role: PassRole,
    axis: ProfileAxisName,
) -> FormulaConstant {
    let profile = operation_default_profile(family, role);
    match axis {
        ProfileAxisName::Ap => profile_row(family, role, "ap", profile.ap_factor),
        ProfileAxisName::Ae => profile_row(family, role, "ae", profile.ae_factor),
    }
}

/// P10 (ruling D8, lead gate A): the radial width (mm) that `calculate`
/// reads on an unstated radial axis, with its basis: the family `ae_factor`
/// after the tool, flute, material and machine adjustments of
/// [`default_engagement`], times D. The add door's stepover rule
/// (`compute::geometry_default`) reads this ONE function, so a new op starts
/// on the calculator's own width and no repo check warns on it.
#[must_use]
pub fn default_radial_width(input: &FeedsInput, machine: &MachineProfile) -> (f64, Basis) {
    let profile = operation_default_profile(input.operation, input.pass_role);
    let (_, ae, _, ae_basis) = default_engagement(input.tool_diameter, &profile, input, machine);
    (ae, ae_basis)
}

/// The family default engagement `(ap, ae)` and the basis of each
/// (rows 8 to 12 of the P5 card).
fn default_engagement(
    d: f64,
    profile: &DefaultProfile,
    input: &FeedsInput,
    machine: &MachineProfile,
) -> (f64, f64, Basis, Basis) {
    let mut ap_factor = profile.ap_factor;
    let mut ae_factor = profile.ae_factor;
    let mut ap_basis = Basis::Rule(profile_row(
        input.operation,
        input.pass_role,
        "ap",
        ap_factor,
    ));
    let mut ae_basis = Basis::Rule(profile_row(
        input.operation,
        input.pass_role,
        "ae",
        ae_factor,
    ));

    // --- Adaptive-specific engagement matrix per tool geometry ---
    // From reference calcs.rs: separate entries for Flat/Ball/TaperedBall.
    // F201: that file is not in the repo; every factor below is a marked
    // unsourced constant (`ADAPTIVE_*`).
    if input.operation == OperationFamily::Adaptive {
        let roughing = input.pass_role == PassRole::Roughing;
        let semi = input.pass_role == PassRole::SemiFinish;
        if roughing || semi {
            let (ap_base, ae_base) = match input.tool_geometry {
                ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } => {
                    ap_basis = Basis::Rule(ADAPTIVE_FLAT_AP);
                    ae_basis = Basis::Rule(ADAPTIVE_FLAT_AE);
                    (ADAPTIVE_FLAT_AP.value, ADAPTIVE_FLAT_AE.value)
                }
                ToolGeometryHint::Ball => {
                    ap_basis = Basis::Rule(ADAPTIVE_BALL_AP);
                    ae_basis = Basis::Rule(ADAPTIVE_BALL_AE);
                    (ADAPTIVE_BALL_AP.value, ADAPTIVE_BALL_AE.value)
                }
                ToolGeometryHint::TaperedBall { .. } => {
                    ap_basis = Basis::Rule(ADAPTIVE_TAPERED_AP);
                    ae_basis = Basis::Rule(ADAPTIVE_TAPERED_AE);
                    (ADAPTIVE_TAPERED_AP.value, ADAPTIVE_TAPERED_AE.value)
                }
                ToolGeometryHint::VBit { .. } => (ap_factor, ae_factor),
            };
            ap_factor = if roughing {
                ap_base
            } else {
                ap_basis = Basis::derived(
                    "adaptive semi-finish scale",
                    vec![ap_basis, Basis::Rule(ADAPTIVE_SEMI_AP_SCALE)],
                );
                ap_base * ADAPTIVE_SEMI_AP_SCALE.value
            };
            ae_factor = if roughing {
                ae_base
            } else {
                ae_basis = Basis::derived(
                    "adaptive semi-finish scale",
                    vec![ae_basis, Basis::Rule(ADAPTIVE_SEMI_AE_SCALE)],
                );
                ae_base * ADAPTIVE_SEMI_AE_SCALE.value
            };

            // Multi-flute AE derate for adaptive (>= 3 flutes)
            if input.flute_count >= 3 {
                ae_factor *= ADAPTIVE_MULTI_FLUTE_AE_SCALE.value;
                ae_basis = Basis::derived(
                    "adaptive flute scale",
                    vec![ae_basis, Basis::Rule(ADAPTIVE_MULTI_FLUTE_AE_SCALE)],
                );
            }

            // Feed-scale-dependent adaptive derates — harder materials
            // (higher factor) want shallower ap/ae; softer materials
            // can take a slightly more aggressive bite.
            let feed_scale = input.material.feed_scale_factor();
            let material_scale = if feed_scale > ADAPTIVE_HARD_THRESHOLD.value {
                Some((ADAPTIVE_HARD_AP_SCALE, ADAPTIVE_HARD_AE_SCALE))
            } else if feed_scale > ADAPTIVE_MEDIUM_THRESHOLD.value {
                Some((ADAPTIVE_MEDIUM_AP_SCALE, ADAPTIVE_MEDIUM_AE_SCALE))
            } else if feed_scale < ADAPTIVE_SOFT_THRESHOLD.value {
                Some((ADAPTIVE_SOFT_AP_SCALE, ADAPTIVE_SOFT_AE_SCALE))
            } else {
                None
            };
            if let Some((ap_scale, ae_scale)) = material_scale {
                ap_factor *= ap_scale.value;
                ae_factor *= ae_scale.value;
                ap_basis = Basis::derived(
                    "adaptive material scale",
                    vec![ap_basis, Basis::Rule(ap_scale)],
                );
                ae_basis = Basis::derived(
                    "adaptive material scale",
                    vec![ae_basis, Basis::Rule(ae_scale)],
                );
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
        let rigidity_ap_floor = machine.rigidity.adaptive_doc_factor * profile.ap_factor
            / ADAPTIVE_RIGIDITY_DOC_DIVISOR.value;
        if rigidity_ap_floor > ap_factor {
            ap_basis = Basis::derived(
                "rigidity floor",
                vec![
                    rigidity_basis(machine),
                    Basis::Rule(ADAPTIVE_RIGIDITY_DOC_DIVISOR),
                    ap_basis,
                ],
            );
        }
        ap_factor = ap_factor.max(rigidity_ap_floor);
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
        let ae_before = ae_factor;
        if wood_class_flat_tool {
            ae_factor = ae_factor.max(machine.rigidity.adaptive_woc_factor);
        }
        ae_factor = ae_factor.min(machine.rigidity.adaptive_woc_factor);
        ae_basis = moved_basis(
            ae_basis,
            ae_before,
            ae_factor,
            "rigidity width",
            rigidity_basis(machine),
        );
    }

    // --- Tool geometry adjustments for finishing operations ---
    match (input.tool_geometry, input.operation, input.pass_role) {
        (
            ToolGeometryHint::Ball,
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::Finish,
        ) => {
            ap_factor = FINISH_BALL_AP.value;
            ae_factor = FINISH_BALL_AE.value;
            ap_basis = Basis::Rule(FINISH_BALL_AP);
            ae_basis = Basis::Rule(FINISH_BALL_AE);
        }
        (
            ToolGeometryHint::Ball,
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::SemiFinish,
        ) => {
            ap_factor = SEMI_FINISH_BALL_AP.value;
            ae_factor = SEMI_FINISH_BALL_AE.value;
            ap_basis = Basis::Rule(SEMI_FINISH_BALL_AP);
            ae_basis = Basis::Rule(SEMI_FINISH_BALL_AE);
        }
        (
            ToolGeometryHint::TaperedBall { .. },
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::Finish,
        ) => {
            ap_factor = FINISH_TAPERED_AP.value;
            ae_factor = FINISH_TAPERED_AE.value;
            ap_basis = Basis::Rule(FINISH_TAPERED_AP);
            ae_basis = Basis::Rule(FINISH_TAPERED_AE);
        }
        (
            ToolGeometryHint::TaperedBall { .. },
            OperationFamily::Parallel | OperationFamily::Scallop,
            PassRole::SemiFinish,
        ) => {
            ap_factor = SEMI_FINISH_TAPERED_AP.value;
            ae_factor = SEMI_FINISH_TAPERED_AE.value;
            ap_basis = Basis::Rule(SEMI_FINISH_TAPERED_AP);
            ae_basis = Basis::Rule(SEMI_FINISH_TAPERED_AE);
        }
        _ => {}
    }

    let ap = (d * ap_factor).max(MIN_AP_MM);
    let ae = (d * ae_factor).max(MIN_AE_MM);
    (
        ap,
        ae,
        Basis::derived("factor x D", vec![ap_basis, Basis::tool_diameter()]),
        Basis::derived("factor x D", vec![ae_basis, Basis::tool_diameter()]),
    )
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
/// Made `pub` on 2026-08-19 for a Suggest pass 9 sentry. Feeds contract P5
/// deleted that sentry (`suggest_feed_matches_final_geometry`). Do not add
/// a second copy of the chip-thinning diameter: C3 retired that copy.
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
