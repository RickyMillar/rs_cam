//! Per-applied-value provenance for the feeds stored on an operation.
//!
//! W2.1 (IA cleanup, Tier 2). Before this module the feeds UI recomputed a
//! *fresh* LUT lookup every frame to colour its pills, and reused the single
//! [`ChiploadSource`](super::ChiploadSource) — which describes only the
//! *chipload's* origin — to label all of feed / plunge / RPM / DOC / WOC. The
//! result: a vendor RPM with a formula-fallback chipload rendered amber
//! ("formula fallback") on every field, and a hand-edited value still showed
//! whatever a fresh lookup would derive rather than "manual".
//!
//! [`FeedsProvenance`] fixes both: it records, per feeds dimension, *how the
//! stored value was produced* — stamped at write time, persisted with the
//! operation, and read back instead of recomputed. [`ChiploadSource`] keeps its
//! narrow role as the chipload's own origin inside
//! [`FeedsResult`](super::FeedsResult); it is no longer the universal label.

use serde::{Deserialize, Serialize};

use super::{ChiploadSource, FeedsResult};
use crate::compute::catalog::OperationConfig;

/// How a single stored feeds value came to be.
///
/// `VendorLut` / `Formula` / `EdgeRadiusFloor` mirror the suggest calculator's
/// derivation (and, for a single suggest pass, can differ *per field* — a row
/// may publish an RPM while its chipload falls back to the formula). `Manual`,
/// `Optimizer`, and `AutoCorrect` record the non-suggest producers.
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
    /// Set by the tool-load optimizer.
    Optimizer,
    /// Set by automatic stale-default / defect correction.
    AutoCorrect,
}

/// Provenance of one stored feeds value.
///
/// `reference` carries a source-specific pointer — for [`ProvenanceSource::VendorLut`]
/// this is the matched observation id. It is `None` for every other source.
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

    /// Bare source with no reference (formula / floor / manual / optimizer / auto-correct).
    pub fn bare(source: ProvenanceSource) -> Self {
        Self {
            source,
            reference: None,
        }
    }

    pub fn formula() -> Self {
        Self::bare(ProvenanceSource::Formula)
    }

    pub fn edge_radius_floor() -> Self {
        Self::bare(ProvenanceSource::EdgeRadiusFloor)
    }

    pub fn manual() -> Self {
        Self::bare(ProvenanceSource::Manual)
    }

    pub fn optimizer() -> Self {
        Self::bare(ProvenanceSource::Optimizer)
    }

    pub fn auto_correct() -> Self {
        Self::bare(ProvenanceSource::AutoCorrect)
    }

    /// Map the chipload's own origin onto the value(s) it derives (feed, plunge,
    /// and the chipload itself).
    pub fn from_chipload_source(source: &ChiploadSource) -> Self {
        match source {
            ChiploadSource::VendorLut { observation_id } => {
                Self::vendor_lut(observation_id.clone())
            }
            ChiploadSource::FormulaFallback => Self::formula(),
            ChiploadSource::EdgeRadiusFloor => Self::edge_radius_floor(),
        }
    }
}

/// Per-dimension provenance for the feeds stored on a `ToolpathConfig`.
///
/// `None` for a field means "never explicitly derived or edited" — the
/// operation's struct default still stands. Each field maps to the
/// corresponding `OperationParams` accessor (so variant-named storage like
/// `WaterlineConfig::z_step` is keyed under `depth_per_pass`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

    /// As [`Self::apply_suggested`] but stamps only the chosen subset — the
    /// SPEED dimensions (feed / plunge / RPM) when `speeds`, the CUT dimensions
    /// (stepover / DOC) when `geometry`. Backs the W3.1 speed-only / cut-only
    /// applies so a speed apply never restamps the geometry provenance.
    pub fn apply_suggested_subset(
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

    /// Stamp a single dimension. Used by per-field GUI/controller apply and by
    /// manual / optimizer / auto-correct write sites.
    pub fn set(&mut self, field: FeedsField, prov: ValueProvenance) {
        *self.slot_mut(field) = Some(prov);
    }

    /// Detect in-place manual edits at the GUI's entry→session flush and stamp
    /// them [`ProvenanceSource::Manual`] (W2.1).
    ///
    /// The GUI edits a transient `ToolpathEntry` copy each frame; numeric
    /// DragValues mutate the operation directly without touching provenance,
    /// whereas Suggest/optimizer paths update *both*. So at the single flush
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
    }

    /// Stamp [`ProvenanceSource::Optimizer`] on every feeds dimension whose
    /// value differs between `baseline` and `candidate`; dimensions the
    /// optimizer left untouched keep their existing provenance. Used when the
    /// tool-load optimizer's chosen candidate is applied to a toolpath.
    #[must_use]
    pub fn stamped_optimizer(
        mut self,
        baseline: &OperationConfig,
        candidate: &OperationConfig,
    ) -> Self {
        let b = baseline.as_params();
        let c = candidate.as_params();
        if b.feed_rate() != c.feed_rate() {
            self.feed_rate = Some(ValueProvenance::optimizer());
        }
        if b.plunge_rate() != c.plunge_rate() {
            self.plunge_rate = Some(ValueProvenance::optimizer());
        }
        if b.spindle_rpm() != c.spindle_rpm() {
            self.spindle_rpm = Some(ValueProvenance::optimizer());
        }
        if b.stepover() != c.stepover() {
            self.stepover = Some(ValueProvenance::optimizer());
        }
        if b.depth_per_pass() != c.depth_per_pass() {
            self.depth_per_pass = Some(ValueProvenance::optimizer());
        }
        if b.scallop_height() != c.scallop_height() {
            self.scallop_height = Some(ValueProvenance::optimizer());
        }
        self
    }

    fn slot_mut(&mut self, field: FeedsField) -> &mut Option<ValueProvenance> {
        match field {
            FeedsField::FeedRate => &mut self.feed_rate,
            FeedsField::PlungeRate => &mut self.plunge_rate,
            FeedsField::SpindleRpm => &mut self.spindle_rpm,
            FeedsField::Stepover => &mut self.stepover,
            FeedsField::DepthPerPass => &mut self.depth_per_pass,
            FeedsField::ScallopHeight => &mut self.scallop_height,
        }
    }
}

/// The feeds dimensions that carry independent provenance.
///
/// `ScallopHeight` rounds out the set for symmetry with [`FeedsProvenance`]'s
/// fields; it is currently only reachable via the struct-field write paths
/// (`detect_manual_edits` / `stamped_optimizer`), not [`FeedsProvenance::set`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedsField {
    FeedRate,
    PlungeRate,
    SpindleRpm,
    Stepover,
    DepthPerPass,
    #[allow(dead_code)] // symmetry; scallop is stamped via struct fields, not set()
    ScallopHeight,
}

impl FeedsResult {
    /// Per-field provenance for every value a suggest pass produces.
    ///
    /// Full per-field independence (W2.1): feed / plunge / chipload inherit the
    /// [`ChiploadSource`]; RPM, axial DOC, and radial WOC are each labelled
    /// `VendorLut` only when the matched row actually published that quantity
    /// (`rpm_*`, `ap_*`, `ae_*` respectively), otherwise `Formula`. `scallop_height`
    /// is an operator input, never suggested, so it is left `None` here.
    pub fn provenance(&self) -> FeedsProvenance {
        let chip = ValueProvenance::from_chipload_source(&self.chipload_source);

        let row_vendor_or_formula = |row_supplies: bool| -> ValueProvenance {
            match self.matched_lut_row.as_ref() {
                Some(row) if row_supplies => {
                    ValueProvenance::vendor_lut(row.observation_id.clone())
                }
                _ => ValueProvenance::formula(),
            }
        };

        let rpm = row_vendor_or_formula(self.matched_lut_row.as_ref().is_some_and(|r| {
            r.rpm_nominal.is_some() || r.rpm_min.is_some() || r.rpm_max.is_some()
        }));
        let doc = row_vendor_or_formula(self.matched_lut_row.as_ref().is_some_and(|r| {
            r.ap_min_mm.is_some()
                || r.ap_max_mm.is_some()
                || r.ap_min_factor.is_some()
                || r.ap_max_factor.is_some()
        }));
        let woc = row_vendor_or_formula(
            self.matched_lut_row
                .as_ref()
                .is_some_and(|r| r.ae_min_mm.is_some() || r.ae_max_mm.is_some()),
        );

        FeedsProvenance {
            feed_rate: Some(chip.clone()),
            plunge_rate: Some(chip),
            spindle_rpm: Some(rpm),
            stepover: Some(woc),
            depth_per_pass: Some(doc),
            scallop_height: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
            is_extrapolated: false,
            row_pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
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
            ramp_feed_mm_min: 500.0,
            axial_depth_mm: 4.0,
            radial_width_mm: 1.0,
            power_kw: 0.3,
            available_power_kw: 0.6,
            power_limited: false,
            mrr_mm3_min: 1200.0,
            warnings: vec![],
            vendor_source: None,
            chipload_source,
            chipload_bounds: None,
            matched_lut_row,
            effective_diameter_mm: 6.0,
            derates: FeedsDerates::default(),
        }
    }

    /// The headline W2.1 bug: a vendor RPM with a formula-fallback chipload must
    /// read VendorLut on RPM and Formula on feed — not the same overloaded label.
    #[test]
    fn provenance_is_per_field_independent() {
        let mut row = lut_row();
        row.rpm_nominal = Some(18_000.0); // row publishes an RPM…
        // …but no ap_*/ae_* and chipload fell back to the formula.
        let result = feeds_result(ChiploadSource::FormulaFallback, Some(row));
        let p = result.provenance();

        assert_eq!(p.feed_rate.unwrap().source, ProvenanceSource::Formula);
        assert_eq!(p.plunge_rate.unwrap().source, ProvenanceSource::Formula);
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
        let result = feeds_result(
            ChiploadSource::VendorLut {
                observation_id: "obs-123".to_owned(),
            },
            Some(row),
        );
        let p = result.provenance();
        assert_eq!(p.feed_rate.unwrap().source, ProvenanceSource::VendorLut);
        assert_eq!(
            p.depth_per_pass.unwrap().source,
            ProvenanceSource::VendorLut
        );
        assert_eq!(p.stepover.unwrap().source, ProvenanceSource::VendorLut);
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
        let old_op = OperationConfig::new_default(OperationType::Pocket);
        let mut new_op = old_op.clone();
        new_op.set_feed_rate(old_op.feed_rate() + 500.0); // hand-edited feed
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
        let old_op = OperationConfig::new_default(OperationType::Pocket);
        let mut new_op = old_op.clone();
        new_op.set_feed_rate(old_op.feed_rate() + 500.0);

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
