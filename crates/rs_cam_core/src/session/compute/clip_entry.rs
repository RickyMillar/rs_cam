//! P29-8e: the entry step after the clips, and the two entry facts that the
//! dressup pipeline and this step share (the surface probe and the helix
//! containment).
//!
//! The boundary clip and the obstacle clip carry their re-entry descents
//! (`geometry::boundary::Reentering`). This stage is the one place that
//! turns them back into a toolpath for generation: it sends them through
//! the op's entry rule (`compute::execute::enter_clip_reentries`), or it
//! leaves them to the rest arm.

use super::JobPhases;
use crate::tool::MillingCutter as _;

/// Who owns the re-entry descents of the clips of one op.
enum ReentryOwner {
    /// The entry step: the op's entry rule. The op has no surface probe, so
    /// its ramp is the fold along the kept cut (the only ramp the step
    /// emits).
    EntryRule,
    /// A rest-driven surface pass: the entry-descent split ramps them on
    /// the rest stock (`dressup::RestEntryRamp`).
    RestRamp,
    /// A surface-riding op on fresh stock (lead ruling Q10): the re-entry
    /// stays the straight plunge.
    SurfaceProbe,
}

impl JobPhases<'_, '_> {
    /// The owner of the clip re-entries. A surface-riding op names
    /// `entry_probe_leave`; `RestEntryRamp` runs exactly when it does and
    /// the generator was seeded with a rest stock.
    fn reentry_owner(&self) -> ReentryOwner {
        match (self.context.entry_probe_leave, self.gen_initial_stock) {
            (Some(_), Some(_)) => ReentryOwner::RestRamp,
            (Some(_), None) => ReentryOwner::SurfaceProbe,
            (None, Some(_) | None) => ReentryOwner::EntryRule,
        }
    }

    /// THE entry step after the clips (P29-8e). It reads the entry that
    /// the dressups resolved (`entry`) and works no style or feed out
    /// again (lead ruling Q6).
    pub(super) fn enter_clip_reentries(
        &self,
        clipped: crate::geometry::boundary::Reentering,
        entry: &crate::compute::execute::PipelineEntry,
        channels: &mut crate::trace::transform_provenance::ReconcileSet<'_>,
    ) -> crate::compute::execute::GeneratedToolpath {
        match self.reentry_owner() {
            ReentryOwner::RestRamp => {
                clipped.stay_plunges(crate::geometry::boundary::PlungesStay::RestEntryRamp)
            }
            ReentryOwner::SurfaceProbe => {
                clipped.stay_plunges(crate::geometry::boundary::PlungesStay::SurfaceProbe)
            }
            ReentryOwner::EntryRule => {
                let entry_containment = self.entry_containment_regions();
                crate::compute::execute::enter_clip_reentries(
                    clipped,
                    entry,
                    crate::compute::execute::DressupContext {
                        cfg: &self.context.dressups,
                        material: &self.context.material,
                        nominal_feed_rate: self.context.feed_rate,
                        plunge_rate_mm_min: self.context.plunge_rate,
                        ramp_feed_rate_mm_min: self.context.ramp_feed_rate,
                        tool_diameter: self.inputs.tool_def.diameter(),
                        safe_z: self.inputs.heights.retract_z,
                        stock_top: self.inputs.emission_stock_bbox.max.z,
                        prior_stock: self.context.prior_stock.as_deref(),
                        link_kinematics: self.context.link_kinematics.as_ref(),
                        op_cutting_ceiling: Some(self.context.op_cutting_ceiling),
                        feed_opt_stock: None,
                        cutter: Some(&self.inputs.tool_def as &dyn crate::tool::MillingCutter),
                        // Q10: the owner is `EntryRule` only with no
                        // surface probe, so the step's ramp is the fold.
                        entry_surface: None,
                        transform_capabilities: self.context.transform_capabilities,
                        entry: self.context.entry_contract,
                        debug_ctx: None,
                        semantic_ctx: None,
                        entry_containment: entry_containment.as_deref(),
                    },
                    channels,
                )
            }
        }
    }

    /// G-RAMPTERRAIN: entry moves of SURFACE-RIDING operations clip to the
    /// drop-cutter surface (`entry_probe_leave` names them). Prism
    /// operations get no probe: their entries legitimately descend below
    /// the model surface (FINDINGS.md amendment 1).
    pub(super) fn entry_surface_probe(&self) -> Option<crate::dressup::EntrySurfaceProbe<'_>> {
        match (
            self.inputs.mesh.as_deref(),
            self.inputs.spatial_index(),
            self.context.entry_probe_leave,
        ) {
            (Some(m), Some(idx), Some(leave)) => Some(crate::dressup::EntrySurfaceProbe {
                mesh: m,
                index: idx,
                cutter: &self.inputs.tool_def,
                stock_to_leave: leave,
                off_mesh: crate::dressup::OffMeshEntry::PlungeFallback,
                // G-ISOCLIPENTRY: `Some` only on a rest-driven pass —
                // `gen_initial_stock` is `None` for `StockSource::Fresh`.
                rest_stock: self.gen_initial_stock,
            }),
            _ => None,
        }
    }

    /// G-ADAPTPASSLOAD round 3: 2D Adaptive's helix entries stay inside the
    /// machinable region (the same region its planner stamps from). The
    /// regions are the view the prologue gave the op: `view` is a pure
    /// function of the resolved selection and the item use.
    pub(super) fn entry_containment_regions(&self) -> Option<Vec<crate::polygon::Polygon2>> {
        self.context
            .transform_capabilities
            .helix_floor_lap
            .then(|| {
                crate::selection::view(
                    &self.inputs.selection,
                    self.inputs.operation.capabilities().item_use,
                )
                .ok()
            })
            .flatten()
            .and_then(|(items, _)| {
                let r = self.inputs.tool_def.radius();
                items.regions().map(|regions| {
                    regions
                        .as_slice()
                        .iter()
                        .flat_map(|p| crate::adaptive::tool_centre_pieces(p, r))
                        .collect()
                })
            })
    }
}
