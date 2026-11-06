//! Per-applied-value provenance for the feeds stored on an operation.
//!
//! W2.1 (IA cleanup, Tier 2). Before this module the feeds UI recomputed a
//! *fresh* LUT lookup every frame to colour its pills, and reused the single
//! [`super::ChiploadSource`] — which describes only the
//! *chipload's* origin — to label all of feed / plunge / RPM / DOC / WOC. The
//! result: a vendor RPM with a formula-fallback chipload rendered amber
//! ("formula fallback") on every field, and a hand-edited value still showed
//! whatever a fresh lookup would derive rather than "manual".
//!
//! [`FeedsProvenance`] fixes both: it records, per feeds dimension, *how the
//! stored value was produced* — stamped at write time, persisted with the
//! operation, and read back instead of recomputed. [`ChiploadSource`] keeps its
//! narrow role as the chipload's own origin inside
//! [`super::FeedsResult`]; it is no longer the universal label.

use serde::{Deserialize, Serialize};

use super::ramp::{RampArm, RampFeed};
use super::sourced::ClaimKind;
use super::{Basis, ChiploadSource, FeedsResult};
use crate::compute::catalog::OperationConfig;
use crate::material::ConstantSource;

/// How a single stored feeds value came to be.
///
/// `VendorLut` / `Formula` / `EdgeRadiusFloor` mirror the suggest calculator's
/// derivation (and, for a single suggest pass, can differ *per field* — a row
/// may publish an RPM while its chipload falls back to the formula). `Manual`
/// and `AutoCorrect` record the non-suggest producers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceSource {
    /// Derived from a matched vendor-LUT observation row.
    VendorLut,
    /// Derived from the empirical feeds formula (no vendor row supplied this field).
    Formula,
    /// Pinned to the edge-radius chipload floor.
    EdgeRadiusFloor,
    /// Hand-edited by the user (GUI DragValue or MCP/programmatic set-param).
    Manual,
    /// Set by automatic stale-default / defect correction.
    AutoCorrect,
    /// Set by a published rule with evidence (G10): the reference is the
    /// rule id, for example `g10_plunge_flat`.
    PublishedRule,
    /// Set by a named repo rule with no source (G10): the reference is the
    /// rule name, for example `material_plunge_base` or `ball_tip_cap`.
    RepoRule,
}

/// Provenance of one stored feeds value.
///
/// `reference` carries a source-specific pointer — for [`ProvenanceSource::VendorLut`]
/// this is the matched observation id, for [`ProvenanceSource::PublishedRule`] the
/// rule id, and for [`ProvenanceSource::RepoRule`] the rule name. It is `None` for
/// every other source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueProvenance {
    pub source: ProvenanceSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl ValueProvenance {
    /// Vendor-LUT origin with the matched observation id.
    pub fn vendor_lut(observation_id: impl Into<String>) -> Self {
        Self {
            source: ProvenanceSource::VendorLut,
            reference: Some(observation_id.into()),
        }
    }

    /// Bare source with no reference (formula / floor / manual / auto-correct).
    pub fn bare(source: ProvenanceSource) -> Self {
        Self {
            source,
            reference: None,
        }
    }

    pub fn formula() -> Self {
        Self::bare(ProvenanceSource::Formula)
    }

    fn edge_radius_floor() -> Self {
        Self::bare(ProvenanceSource::EdgeRadiusFloor)
    }

    pub fn manual() -> Self {
        Self::bare(ProvenanceSource::Manual)
    }

    pub fn auto_correct() -> Self {
        Self::bare(ProvenanceSource::AutoCorrect)
    }

    /// R1 (2026-10-30): true when Suggest wrote the value (a vendor row, the
    /// formula, the edge-radius floor or a G10 rule), false for a typed
    /// value (`Manual`) and an automatic correction (`AutoCorrect`). The GUI
    /// marks a suggested value with ⚡.
    #[must_use]
    pub fn is_suggested(&self) -> bool {
        !matches!(
            self.source,
            ProvenanceSource::Manual | ProvenanceSource::AutoCorrect
        )
    }

    /// A published rule with evidence (G10), with the rule id.
    pub fn published_rule(rule_id: impl Into<String>) -> Self {
        Self {
            source: ProvenanceSource::PublishedRule,
            reference: Some(rule_id.into()),
        }
    }

    /// A named repo rule with no source (G10), with the rule name.
    pub fn repo_rule(rule_name: impl Into<String>) -> Self {
        Self {
            source: ProvenanceSource::RepoRule,
            reference: Some(rule_name.into()),
        }
    }

    /// The chipload's own origin, for a chip with no matched row to name
    /// (the edge-radius floor). Every other stamp is a projection of the
    /// basis ([`Self::of_basis`]).
    fn from_chipload_source(source: &ChiploadSource) -> Self {
        match source {
            ChiploadSource::VendorLut { observation_id } => {
                Self::vendor_lut(observation_id.clone())
            }
            ChiploadSource::FormulaFallback => Self::formula(),
            ChiploadSource::EdgeRadiusFloor => Self::edge_radius_floor(),
        }
    }

    /// CONTRACT K2 (feeds contract P5): the stamp of a value is a projection
    /// of its [`Basis`], by one exhaustive `match`.
    ///
    /// - The operator: `Manual`; a pick on the envelope carries the
    ///   reference `picked`.
    /// - A printed row, or a claim on a row (G1, G2, G3, G6): `VendorLut`
    ///   with the row id. A G10 claim: `PublishedRule` with the rule id.
    /// - A named rule: `PublishedRule` when Cited, `RepoRule` when
    ///   Unsourced, with the rule name.
    /// - The machine profile: `Formula` with the field name.
    /// - A tool fact (P13a): `RepoRule` with the legacy row name when the
    ///   fact is `Unknown`, else `Formula`.
    /// - A law: the stamp of its first input when that is a vendor row,
    ///   else `Formula` (the law is the repo's formula).
    #[must_use]
    pub fn of_basis(basis: &Basis) -> Self {
        match basis {
            Basis::Operator { law } => Self {
                source: ProvenanceSource::Manual,
                reference: (*law == crate::feeds::sourced::LAW_PICKED)
                    .then(|| crate::feeds::sourced::LAW_PICKED.to_owned()),
            },
            Basis::Printed(row) => Self::vendor_lut(row.id()),
            Basis::Claim(claim) => match claim.kind {
                ClaimKind::PlungeG10 => Self::published_rule(claim.rule.clone()),
                ClaimKind::SizeG1
                | ClaimKind::HardnessG2
                | ClaimKind::FamilyG3
                | ClaimKind::DrillG6 => Self::vendor_lut(claim.row.id()),
            },
            Basis::Rule(constant) => match constant.source {
                ConstantSource::Cited { .. } => Self::published_rule(constant.name.clone()),
                ConstantSource::Unsourced { .. } => Self::repo_rule(constant.name.clone()),
            },
            Basis::Machine(field) => Self {
                source: ProvenanceSource::Formula,
                reference: Some(field.name().to_owned()),
            },
            Basis::ToolFact(fact) => match fact.legacy_row() {
                Some(row) => Self::repo_rule(row.name.clone()),
                None => Self::formula(),
            },
            Basis::Derived { from, .. } => match from.first().map(Self::of_basis) {
                Some(first) if first.source == ProvenanceSource::VendorLut => first,
                Some(_) | None => Self::formula(),
            },
        }
    }
}

/// Per-dimension provenance for the feeds stored on a `ToolpathConfig`.
///
/// `None` for a field means "never explicitly derived or edited" — the
/// operation's struct default still stands. Each field maps to the
/// corresponding `OperationParams` accessor (so variant-named storage like
/// `WaterlineConfig::z_step` is keyed under `depth_per_pass`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedsProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_rate: Option<ValueProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plunge_rate: Option<ValueProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<ValueProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stepover: Option<ValueProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_per_pass: Option<ValueProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scallop_height: Option<ValueProvenance>,
    /// The helix and ramp entry feed (G6 ramp). Suggest stamps it with
    /// [`Self::stamp_ramp`]; a hand edit stamps `Manual`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_feed_rate: Option<ValueProvenance>,
    /// P10 (ruling D8): the peck depth of a drill cycle. The tool rule
    /// stamps it (`compute::geometry_default`); a hand edit stamps `Manual`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peck_depth: Option<ValueProvenance>,
    /// P10: the `z_step` of an op whose depth-per-pass role is not
    /// `z_step` (Steep and Shallow, Unified Finish). Waterline's `z_step`
    /// IS its depth per pass, so it is stamped under `depth_per_pass`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_step: Option<ValueProvenance>,
    /// P13b (K8, ruling Q7): the step records of the stored values, THE
    /// store. The producer that writes a value replaces the records in the
    /// same call (the apply funnel, the envelope pick, the add-door rules);
    /// `Lineage::of` reads them. The project file stores them as one basis
    /// table per toolpath (`[...feeds_provenance.steps]`, `nodes` and
    /// `records`; lead ruling 1, stored form C).
    #[serde(
        default,
        skip_serializing_if = "crate::trace::lineage::StepRecords::is_empty"
    )]
    pub steps: crate::trace::lineage::StepRecords,
}

impl FeedsProvenance {
    /// Overwrite the suggested dimensions with the per-field provenance of a
    /// freshly-applied [`FeedsResult`]. Only stamps a field the operation
    /// actually carries (so a Profile with no stepover keeps `stepover: None`),
    /// and only stamps RPM when one was written. `scallop_height` is never
    /// touched — it is an operator input, not a suggested value.
    pub fn apply_suggested(
        &mut self,
        result: &FeedsResult,
        operation: &OperationConfig,
        rpm_written: bool,
    ) {
        self.apply_suggested_subset(result, operation, rpm_written, true, true);
    }

    /// As [`Self::apply_suggested`] but stamps only the dimensions a write
    /// sets: the SPEED dimensions (feed / plunge / RPM) when `speeds`, the
    /// CUT dimensions (stepover / DOC) when `geometry`. A speed apply never
    /// restamps the geometry provenance. The apply funnel passes its
    /// `ChainWrite` arm's two answers (P5: the public scope is gone).
    pub(crate) fn apply_suggested_subset(
        &mut self,
        result: &FeedsResult,
        operation: &OperationConfig,
        rpm_written: bool,
        speeds: bool,
        geometry: bool,
    ) {
        let p = result.provenance();
        let params = operation.as_params();
        if speeds {
            self.feed_rate = p.feed_rate;
            self.plunge_rate = p.plunge_rate;
            if rpm_written {
                self.spindle_rpm = p.spindle_rpm;
            }
        }
        if geometry {
            if params.stepover().is_some() {
                self.stepover = p.stepover;
            }
            if params.depth_per_pass().is_some() {
                self.depth_per_pass = p.depth_per_pass;
            }
        }
    }

    /// Stamp the ramp feed from the record Suggest wrote (G6 ramp). Call it
    /// after the feed and the plunge are stamped:
    ///
    /// - a chip term that sets the feed stamps the G6 side row (`VendorLut`);
    /// - the cut feed that sets it copies the feed's stamp;
    /// - the plunge term of a plunge-slope ramp (G10 Q2) copies the plunge's
    ///   stamp, because the ramp then holds its vertical rate at the plunge;
    /// - the no-entry fallback copies the plunge's stamp, because the entry
    ///   then runs at the plunge rate.
    pub fn stamp_ramp(&mut self, ramp: &RampFeed) {
        self.ramp_feed_rate = match ramp {
            RampFeed::Sourced(s) => match s.arm {
                RampArm::ChipTerm => Some(ValueProvenance::vendor_lut(s.drill.side_row.clone())),
                RampArm::CutFeed => self.feed_rate.clone(),
                RampArm::PlungeTerm => self.plunge_rate.clone(),
            },
            RampFeed::PlungeSlope(s) => match s.arm {
                RampArm::PlungeTerm | RampArm::ChipTerm => self.plunge_rate.clone(),
                RampArm::CutFeed => self.feed_rate.clone(),
            },
            RampFeed::NoEntryFeed { .. } => self.plunge_rate.clone(),
        };
    }

    /// Stamp a single dimension. Used by per-field GUI/controller apply and by
    /// manual / auto-correct write sites.
    pub fn set(&mut self, field: FeedsField, prov: ValueProvenance) {
        *self.slot_mut(field) = Some(prov);
    }

    /// Remove the stamp of a single dimension. The R1 seed calls it when it
    /// clears a suggested value that has no basis for the new tool.
    pub fn clear(&mut self, field: FeedsField) {
        *self.slot_mut(field) = None;
    }

    /// Read a single dimension's stamp. `None` means the operation's struct
    /// default still stands for that field (see the type-level doc).
    pub fn get(&self, field: FeedsField) -> Option<&ValueProvenance> {
        match field {
            FeedsField::FeedRate => self.feed_rate.as_ref(),
            FeedsField::PlungeRate => self.plunge_rate.as_ref(),
            FeedsField::SpindleRpm => self.spindle_rpm.as_ref(),
            FeedsField::Stepover => self.stepover.as_ref(),
            FeedsField::DepthPerPass => self.depth_per_pass.as_ref(),
            FeedsField::ScallopHeight => self.scallop_height.as_ref(),
            FeedsField::RampFeedRate => self.ramp_feed_rate.as_ref(),
            FeedsField::PeckDepth => self.peck_depth.as_ref(),
            FeedsField::ZStep => self.z_step.as_ref(),
        }
    }

    /// Detect in-place manual edits at the GUI's entry→session flush and stamp
    /// them [`ProvenanceSource::Manual`] (W2.1).
    ///
    /// The GUI edits a transient `ToolpathEntry` copy each frame; numeric
    /// DragValues mutate the operation directly without touching provenance,
    /// whereas the Suggest paths update *both*. So at the single flush
    /// choke point we can tell them apart: for each dimension whose value
    /// changed (`old_op` → `new_op`) but whose provenance slot did **not** move
    /// (`self` still equals `old`), the change was a hand-edit. `self` is the
    /// entry's provenance; `old` is the session's pre-flush provenance.
    pub fn detect_manual_edits(
        &mut self,
        old_op: &OperationConfig,
        new_op: &OperationConfig,
        old: &FeedsProvenance,
    ) {
        let o = old_op.as_params();
        let n = new_op.as_params();
        let manual = || Some(ValueProvenance::manual());
        if o.feed_rate() != n.feed_rate() && self.feed_rate == old.feed_rate {
            self.feed_rate = manual();
        }
        if o.plunge_rate() != n.plunge_rate() && self.plunge_rate == old.plunge_rate {
            self.plunge_rate = manual();
        }
        if o.spindle_rpm() != n.spindle_rpm() && self.spindle_rpm == old.spindle_rpm {
            self.spindle_rpm = manual();
        }
        if o.stepover() != n.stepover() && self.stepover == old.stepover {
            self.stepover = manual();
        }
        if o.depth_per_pass() != n.depth_per_pass() && self.depth_per_pass == old.depth_per_pass {
            self.depth_per_pass = manual();
        }
        if o.scallop_height() != n.scallop_height() && self.scallop_height == old.scallop_height {
            self.scallop_height = manual();
        }
        if o.ramp_feed_rate() != n.ramp_feed_rate() && self.ramp_feed_rate == old.ramp_feed_rate {
            self.ramp_feed_rate = manual();
        }
        // P10: every tool-derived geometry field, by its stamp slot (a
        // peck, a Z step, a single-pass Pencil offset stepover).
        let old_fields = crate::compute::geometry_default::read_fields(old_op);
        for (field, new_value) in crate::compute::geometry_default::read_fields(new_op) {
            let old_value = old_fields
                .iter()
                .find(|(name, _)| *name == field)
                .and_then(|(_, v)| *v);
            let slot = crate::compute::geometry_default::stamp_slot(new_op, field);
            if old_value != new_value && self.get(slot) == old.get(slot) {
                self.set(slot, ValueProvenance::manual());
            }
        }
    }

    fn slot_mut(&mut self, field: FeedsField) -> &mut Option<ValueProvenance> {
        match field {
            FeedsField::FeedRate => &mut self.feed_rate,
            FeedsField::PlungeRate => &mut self.plunge_rate,
            FeedsField::SpindleRpm => &mut self.spindle_rpm,
            FeedsField::Stepover => &mut self.stepover,
            FeedsField::DepthPerPass => &mut self.depth_per_pass,
            FeedsField::ScallopHeight => &mut self.scallop_height,
            FeedsField::RampFeedRate => &mut self.ramp_feed_rate,
            FeedsField::PeckDepth => &mut self.peck_depth,
            FeedsField::ZStep => &mut self.z_step,
        }
    }
}

/// The feeds dimensions that carry independent provenance.
///
/// `ScallopHeight` rounds out the set for symmetry with [`FeedsProvenance`]'s
/// fields; it is currently only reachable via the struct-field write paths
/// (`detect_manual_edits`), not [`FeedsProvenance::set`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedsField {
    FeedRate,
    PlungeRate,
    SpindleRpm,
    Stepover,
    DepthPerPass,
    /// Kept for symmetry. Scallop height is stamped through the struct
    /// fields, not through `set()`.
    ScallopHeight,
    /// The helix and ramp entry feed (G6 ramp). Suggest writes it through
    /// its own record, so no pill preview carries it.
    RampFeedRate,
    /// P10: the peck depth of a drill cycle (the tool rule stamps it).
    PeckDepth,
    /// P10: the `z_step` of Steep and Shallow and Unified Finish.
    ZStep,
}

impl FeedsResult {
    /// Per-field provenance for every value a suggest pass produces.
    ///
    /// Full per-field independence (W2.1): feed / chipload inherit the
    /// [`ChiploadSource`]; the plunge is stamped by its G10 basis and the term
    /// that binds at this result's feed (`PlungeBasis::provenance`: a claim
    /// stamps `PublishedRule`, the material base and the tip cap stamp
    /// `RepoRule`, the feed and a drill cycle copy the feed stamp); RPM,
    /// axial DOC, and radial WOC are each labelled
    /// `VendorLut` only when the matched row actually published that quantity
    /// (`rpm_*`, `ap_*`, `ae_*` respectively), otherwise `Formula`. `scallop_height`
    /// is an operator input, never suggested, so it is left `None` here.
    pub fn provenance(&self) -> FeedsProvenance {
        // P5 (K2): each stamp is a projection of the field's basis. The
        // "a row with an `ap_*` field labels the depth `VendorLut`" guess is
        // gone: the depth names the rule or the operator that set it.
        let chip = match self.chipload_source {
            ChiploadSource::EdgeRadiusFloor => {
                ValueProvenance::from_chipload_source(&self.chipload_source)
            }
            ChiploadSource::VendorLut { .. } | ChiploadSource::FormulaFallback => {
                ValueProvenance::of_basis(&self.bases.feed)
            }
        };
        let rpm = ValueProvenance::of_basis(&self.bases.rpm);
        let doc = ValueProvenance::of_basis(&self.bases.ap);
        let woc = ValueProvenance::of_basis(&self.bases.ae);

        let plunge =
            match super::plunge::resolve_plunge(&self.plunge, self.feed_rate_mm_min, self.rpm) {
                Some((_, binding)) => self.plunge.provenance(binding, chip.clone()),
                None => chip.clone(),
            };

        FeedsProvenance {
            feed_rate: Some(chip),
            plunge_rate: Some(plunge),
            spindle_rpm: Some(rpm),
            stepover: Some(woc),
            depth_per_pass: Some(doc),
            scallop_height: None,
            // The ramp needs θ and the shipped feed; the apply funnel stamps
            // it (`FeedsProvenance::stamp_ramp`).
            ramp_feed_rate: None,
            // P10: the tool rule stamps these, never the calculator.
            peck_depth: None,
            z_step: None,
            // P13b: the apply funnel writes the step records.
            steps: crate::trace::lineage::StepRecords::default(),
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::catalog::{OperationConfig, OperationType};
    use crate::feeds::vendor_lookup::LookupResult;
    use crate::feeds::{ChiploadSource, FeedsDerates, FeedsResult};

    fn lut_row() -> LookupResult {
        LookupResult {
            chip_load_mm: 0.0,
            chip_load_min_mm: None,
            chip_load_max_mm: None,
            rpm_nominal: None,
            rpm_min: None,
            rpm_max: None,
            ap_min_mm: None,
            ap_max_mm: None,
            ap_min_factor: None,
            ap_max_factor: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "obs-123".to_owned(),
            source_vendor: crate::feeds::vendor_lut::Vendor::Amana,
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            chipload_diameter_ratio_raw: 1.0,
            chipload_hardness_ratio_raw: 1.0,
            is_extrapolated: false,
            row_pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
            size_basis: crate::feeds::extrapolation::SizeBasis::Exact,
            hardness_basis: crate::feeds::extrapolation::HardnessBasis::Unscaled,
            family_basis: crate::feeds::extrapolation::FamilyBasis::Printed,
            drill_basis: crate::feeds::extrapolation::DrillBasis::Printed,
            material_label: String::new(),
            evidence_grade: crate::feeds::vendor_lut::EvidenceGrade::A,
            row_kind: crate::feeds::vendor_lut::ObservationKind::Exact,
            band: crate::feeds::vendor_lookup::VendorBand::alone("obs-123"),
        }
    }

    fn feeds_result(
        chipload_source: ChiploadSource,
        matched_lut_row: Option<LookupResult>,
    ) -> FeedsResult {
        FeedsResult {
            rpm: 18_000.0,
            chip_load_mm: 0.02,
            feed_rate_mm_min: 1000.0,
            plunge_rate_mm_min: 400.0,
            plunge: crate::feeds::PlungeBasis::MaterialBase {
                reason: crate::feeds::extrapolation::PlungeRefusal::NoRuleForFamily(
                    crate::feeds::vendor_lut::ToolFamily::BullNose,
                ),
                base_mm_min: 400.0,
                tip_cap: None,
                axial: None,
            },
            ramp: crate::feeds::RampBasis::NoChip {
                reason: crate::feeds::RampFallback::NoLut,
            },
            axial_depth_mm: 4.0,
            radial_width_mm: 1.0,
            power_kw: 0.3,
            power_case: crate::feeds::engagement::PowerCase::Nominal,
            available_power_kw: 0.6,
            power_limited: false,
            mrr_mm3_min: Ok(1200.0),
            warnings: vec![],
            vendor_source: None,
            support: if matched_lut_row.is_some() {
                crate::feeds::FeedsSupport::VendorBacked
            } else {
                crate::feeds::FeedsSupport::FormulaOnly {
                    source: crate::feeds::support::MILLING_FORMULA_SOURCE,
                }
            },
            chipload_source,
            chipload_bounds: None,
            chipload_point_mm: None,
            matched_lut_row,
            effective_diameter_mm: 6.0,
            derates: FeedsDerates::default(),
            bases: crate::feeds::PointBases::not_computed(),
            steps: Vec::new(),
        }
    }

    /// The headline W2.1 bug: a vendor RPM with a formula-fallback chipload must
    /// read VendorLut on RPM and Formula on feed — not the same overloaded label.
    #[test]
    fn provenance_is_per_field_independent() {
        let mut row = lut_row();
        row.rpm_nominal = Some(18_000.0); // row publishes an RPM…
        // …but no ap_*/ae_* and chipload fell back to the formula.
        let mut result = feeds_result(ChiploadSource::FormulaFallback, Some(row));
        // P5: the stamps are projections of the bases `calculate` writes.
        result.bases.rpm = Basis::Printed(crate::feeds::sourced::RowRef("obs-123".to_owned()));
        let p = result.provenance();

        assert_eq!(p.feed_rate.unwrap().source, ProvenanceSource::Formula);
        // G10: the plunge is stamped by its own basis, not by the chip. The
        // fixture's material base (400) is under the feed (1000), so the
        // named repo rule binds.
        assert_eq!(
            p.plunge_rate.unwrap(),
            ValueProvenance::repo_rule("material_plunge_base")
        );
        let rpm = p.spindle_rpm.unwrap();
        assert_eq!(rpm.source, ProvenanceSource::VendorLut);
        assert_eq!(rpm.reference.as_deref(), Some("obs-123"));
        assert_eq!(p.depth_per_pass.unwrap().source, ProvenanceSource::Formula);
        assert_eq!(p.stepover.unwrap().source, ProvenanceSource::Formula);
        assert!(p.scallop_height.is_none());
    }

    #[test]
    fn provenance_vendor_chipload_labels_feed_vendor() {
        let mut row = lut_row();
        row.ap_min_mm = Some(2.0);
        row.ae_max_mm = Some(3.0);
        let mut result = feeds_result(
            ChiploadSource::VendorLut {
                observation_id: "obs-123".to_owned(),
            },
            Some(row),
        );
        let printed = Basis::Printed(crate::feeds::sourced::RowRef("obs-123".to_owned()));
        result.bases.feed = Basis::derived(
            "chip x RPM x Z x depth tier",
            vec![Basis::derived("band midpoint", vec![printed])],
        );
        result.bases.ap = Basis::Rule(crate::material::FormulaConstant::unsourced(
            "feeds.operation_default_profile.Pocket.Roughing.ap_factor",
            0.7,
            "no record",
        ));
        result.bases.ae = Basis::stated();
        let p = result.provenance();
        assert_eq!(p.feed_rate.unwrap().source, ProvenanceSource::VendorLut);
        // P5 (K2): a row's `ap_*` field no longer labels the depth. The
        // depth names the rule that set it, the width the operator.
        assert_eq!(
            p.depth_per_pass.unwrap(),
            ValueProvenance::repo_rule("feeds.operation_default_profile.Pocket.Roughing.ap_factor")
        );
        assert_eq!(p.stepover.unwrap().source, ProvenanceSource::Manual);
    }

    #[test]
    fn apply_suggested_skips_fields_the_op_lacks() {
        // Trace has feed/plunge but no stepover.
        let op = OperationConfig::new_default(OperationType::Trace);
        assert!(op.as_params().stepover().is_none());
        let result = feeds_result(ChiploadSource::FormulaFallback, None);
        let mut prov = FeedsProvenance::default();
        prov.apply_suggested(&result, &op, true);
        assert!(prov.feed_rate.is_some());
        assert!(prov.spindle_rpm.is_some());
        assert!(prov.stepover.is_none(), "Trace has no stepover to stamp");
    }

    #[test]
    fn detect_manual_edits_stamps_only_value_changes_without_new_provenance() {
        let old_op = crate::compute::fixture_values::fixture_op(OperationType::Pocket);
        let mut new_op = old_op.clone();
        new_op.set_feed_rate(old_op.feed_rate().expect("S11: the fixture sets it") + 500.0); // hand-edited feed
        // depth_per_pass unchanged.

        let old_prov = FeedsProvenance::default();
        let mut entry_prov = old_prov.clone();
        entry_prov.detect_manual_edits(&old_op, &new_op, &old_prov);

        assert_eq!(
            entry_prov.feed_rate.unwrap().source,
            ProvenanceSource::Manual
        );
        assert!(entry_prov.depth_per_pass.is_none());
    }

    #[test]
    fn detect_manual_edits_ignores_suggest_driven_changes() {
        // A suggest pass changed feed AND already stamped its provenance — the
        // diff must NOT relabel it Manual.
        let old_op = crate::compute::fixture_values::fixture_op(OperationType::Pocket);
        let mut new_op = old_op.clone();
        new_op.set_feed_rate(old_op.feed_rate().expect("S11: the fixture sets it") + 500.0);

        let old_prov = FeedsProvenance::default();
        let mut entry_prov = old_prov.clone();
        entry_prov.feed_rate = Some(ValueProvenance::vendor_lut("obs-9")); // suggest stamped it
        entry_prov.detect_manual_edits(&old_op, &new_op, &old_prov);

        assert_eq!(
            entry_prov.feed_rate.unwrap().source,
            ProvenanceSource::VendorLut
        );
    }
}
