//! Maps rs_cam_core types to vendor LUT query types.

use super::vendor_lookup::LookupQuery;
use super::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use super::{FeedsInput, OperationFamily, PassRole};
use crate::compute::catalog::OperationType;
use crate::material::{Material, PlasticHardness};

/// **The single site that decides which vendor-LUT family a query names.**
/// Checkpoint K (a4), 2026-08-13.
///
/// Two operation kinds do not query under their declared `feeds_family`:
///
/// - **`Adaptive3d`** declares `feeds_family: Adaptive` so it shares F&S
///   inputs with 2D adaptive HSM, but its path geometry is closer to
///   pocket-style clearing in wood. The vendor's 2D adaptive rows narrow
///   stepover by design (e.g. 0.95 mm `ae_max` for a Ø6 flat in
///   hardwood) while operators want 2.5–3 mm on Adaptive3d, so it routes
///   to `Pocket` and the pass role is untouched. (G16 §10 sign-off,
///   design doc §1.3. **This wave does not re-open that judgement** — it
///   applies it to both consumers instead of one.)
/// - **`ProjectCurve`** is not a vendor family at all; it is
///   geometrically a 3D contour trace. Ball / tapered-ball tools route to
///   `(Parallel, Finish)`, flat tools to `(Contour, Finish)`, and bull
///   nose / V-bit / facing bits **refuse** — the LUT has no rows for
///   those and inventing a family would be worse than saying so.
///
/// # Why this function exists, and what it cost before it did
///
/// It was `tool_load::chipload::routed_lookup_family`, applied on the
/// gate / optimizer / viewport side and **not** on the Suggest side.
/// A-6's census measured the divergence
/// (`planning/review_2026-08-08/LUT_BOUNDARY_EVIDENCE.md` §2): over
/// 3 024 (query, reroute) pairs, **489 resolved to different rows**
/// (band-maximum ratio ×0.16 … ×6.17, median ×0.98) and **756** were
/// gate refusals, **378** of which left Suggest returning a confident
/// vendor-backed band on a surface where the gate declined to judge at
/// all. Nothing on any screen said the two were talking about different
/// rows. A-5's 1.273× band divergence — recorded under the resolver-pair
/// ledger row F-LUT2 — is this, re-attributed.
///
/// Both `to_lookup_query` (Suggest / Explain) and
/// `tool_load::chipload::matched_chip_envelope` (gate / optimizer /
/// viewport) call this. The reroute can no longer be applied on one side
/// only, because there is only one side.
///
/// `None` is a **refusal**, and callers must render it as
/// "no vendor data", never as a fallback to the unrouted family.
#[must_use]
pub fn lut_query_for(
    operation_kind: OperationType,
    tool_family: ToolFamily,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> Option<(LutOperationFamily, LutPassRole)> {
    if operation_kind == OperationType::Adaptive3d
        && operation_family == LutOperationFamily::Adaptive
    {
        return Some((LutOperationFamily::Pocket, pass_role));
    }
    if operation_kind != OperationType::ProjectCurve {
        return Some((operation_family, pass_role));
    }
    match tool_family {
        ToolFamily::BallNose | ToolFamily::TaperedBallNose => {
            Some((LutOperationFamily::Parallel, LutPassRole::Finish))
        }
        ToolFamily::FlatEnd => Some((LutOperationFamily::Contour, LutPassRole::Finish)),
        ToolFamily::BullNose | ToolFamily::ChamferVbit | ToolFamily::FacingBit => None,
    }
}

/// The cutter classes `lut_query_for` refuses `ProjectCurve` on, named
/// so a refusal message can say **which rows are missing** rather than
/// "no data". Checkpoint K (a4) ruled the refusal in on the Suggest side
/// too, so this string is now operator-facing.
#[must_use]
pub fn missing_project_curve_rows(tool_family: ToolFamily) -> &'static str {
    match tool_family {
        ToolFamily::BullNose => {
            "the vendor LUT publishes no bull-nose contour/parallel rows for a curve-following \
             pass (it has bull-nose adaptive, pocket and scallop rows only)"
        }
        ToolFamily::ChamferVbit => {
            "the vendor LUT publishes no V-bit contour/parallel rows for a curve-following pass \
             (its V-bit rows are v-carve and trace rows, whose engagement is set by depth, not \
             by following a 3D curve at fixed offset)"
        }
        ToolFamily::FacingBit => {
            "the vendor LUT publishes no facing-bit rows outside the Face family"
        }
        ToolFamily::FlatEnd | ToolFamily::BallNose | ToolFamily::TaperedBallNose => {
            "no rows are missing for this cutter class — it is not refused"
        }
    }
}

/// Canonical `feeds::OperationFamily` → `LutOperationFamily` mapping.
/// F3.2: extracted from `to_lookup_query` so the loader's reachability
/// rule and gate-side query builders share one mapping instead of
/// re-deriving it inline (defect class C5).
pub fn op_family_to_lut(family: OperationFamily) -> LutOperationFamily {
    match family {
        OperationFamily::Adaptive => LutOperationFamily::Adaptive,
        OperationFamily::Pocket => LutOperationFamily::Pocket,
        OperationFamily::Contour => LutOperationFamily::Contour,
        OperationFamily::Parallel => LutOperationFamily::Parallel,
        OperationFamily::Scallop => LutOperationFamily::Scallop,
        OperationFamily::Trace => LutOperationFamily::Trace,
        OperationFamily::Face => LutOperationFamily::Face,
        OperationFamily::Drill => LutOperationFamily::Drill,
    }
}

/// Convert a FeedsInput to a LookupQuery for vendor LUT lookup.
///
/// **Returns `None` when [`lut_query_for`] refuses** — today, a
/// `ProjectCurve` on a bull-nose, V-bit or facing cutter. Checkpoint K
/// (a4) ruled that refusal in on the Suggest side as well as the gate's:
/// 378 recommendations that used to carry a confident vendor band on a
/// surface the gate would not judge become honest no-vendor-data. The
/// caller must surface it, never fall back to the unrouted family.
///
/// When `input.operation_kind` is `None` the routing is a **no-op** and
/// the declared family is used unchanged. That is the pre-a4 behaviour,
/// retained for callers that genuinely have no operation identity (the
/// `FeedsInput`-only unit fixtures); every production Suggest path goes
/// through `suggest::feeds_input_for_operation`, which supplies it.
pub fn to_lookup_query(input: &FeedsInput) -> Option<LookupQuery> {
    let mut query = to_lookup_query_unrouted(input);
    if let Some(kind) = input.operation_kind {
        let (operation_family, pass_role) = lut_query_for(
            kind,
            query.tool_family,
            query.operation_family,
            query.pass_role,
        )?;
        query.operation_family = operation_family;
        query.pass_role = pass_role;
    }
    Some(query)
}

/// The query as the operation **declares** it, with no a4 routing and no
/// refusal.
///
/// This is an **echo**, not a lookup. Its one legitimate use is
/// describing the operation on a surface where no lookup happened — the
/// feeds modal's input chips when the routing refused, or when the caller
/// supplied no LUT at all. Resolving a row through it would reintroduce
/// exactly the one-sided routing a4 removed.
#[must_use]
pub fn to_lookup_query_unrouted(input: &FeedsInput) -> LookupQuery {
    LookupQuery {
        tool_family: input.tool_geometry.cutter_kind().lut_family(),
        tool_subfamily: None,
        diameter_mm: lookup_diameter_for_input(input),
        flute_count: input.flute_count,
        material_family: material_to_lut(input.material).0,
        hardness_kind: Some(material_to_lut(input.material).1),
        hardness_value: Some(material_to_lut(input.material).2),
        operation_family: op_family_to_lut(input.operation),
        pass_role: match input.pass_role {
            PassRole::Roughing => LutPassRole::Roughing,
            PassRole::SemiFinish => LutPassRole::SemiFinish,
            PassRole::Finish => LutPassRole::Finish,
        },
    }
}

fn lookup_diameter_for_input(input: &FeedsInput<'_>) -> f64 {
    let axial_doc = input.axial_depth_mm.unwrap_or(input.tool_diameter).max(0.0);
    input.tool_geometry.engaged_diameter_at_doc(
        axial_doc,
        input.tool_diameter,
        input.shank_diameter.unwrap_or(input.tool_diameter),
    )
}

/// Translate the application-side `Material` into the LUT-side
/// `(MaterialFamily, HardnessKind, hardness_value)` triple a vendor
/// row lookup needs.
///
/// All hardness values come from the canonical accessors on
/// `WoodSpecies` / `PlywoodGrade` / `SheetGoodKind` /
/// `PlasticFamily::hardness()` / `AluminumAlloy::brinell_hb()` so
/// there's exactly ONE source of truth per material. Pre-Phase-1E
/// this function had inline tables that disagreed with the canonical
/// accessors (Acrylic was Shore D 85 here vs Rockwell M 93 on
/// `PlasticFamily::hardness()`).
pub(crate) fn material_to_lut(material: &Material) -> (MaterialFamily, HardnessKind, f64) {
    use crate::material::{PlywoodGrade, SheetGoodKind};
    match material {
        Material::SolidWood { species } => {
            let janka = species.janka_lbf();
            let family = if janka <= 800.0 {
                MaterialFamily::Softwood
            } else {
                MaterialFamily::Hardwood
            };
            (family, HardnessKind::Janka, janka)
        }
        // Parametric solid-wood — Phase E 2026-05-31. Same softwood/
        // hardwood split as the enum variant; the LUT row matcher
        // takes care of band scoring from the actual Janka value.
        Material::SolidWoodByJanka { janka_lbf, .. } => {
            let family = if *janka_lbf <= 800.0 {
                MaterialFamily::Softwood
            } else {
                MaterialFamily::Hardwood
            };
            (family, HardnessKind::Janka, *janka_lbf)
        }
        Material::Plywood { grade } => {
            let family = match grade {
                PlywoodGrade::Softwood => MaterialFamily::PlywoodSoftwood,
                PlywoodGrade::BalticBirch | PlywoodGrade::HardwoodFaced => {
                    MaterialFamily::PlywoodHardwood
                }
            };
            (family, HardnessKind::Janka, grade.effective_janka_lbf())
        }
        Material::SheetGood { kind } => {
            let family = match kind {
                SheetGoodKind::Mdf => MaterialFamily::Mdf,
                SheetGoodKind::Hdf => MaterialFamily::Hdf,
                SheetGoodKind::Particleboard => MaterialFamily::Particleboard,
            };
            (family, HardnessKind::Janka, kind.effective_janka_lbf())
        }
        Material::Plastic { family } => {
            // Per-family LUT material class for row matching. The LUT's
            // MaterialFamily enum has only Acrylic / Hdpe /
            // Polycarbonate / Delrin on the plastic side, so the
            // Phase D (2026-05-31) UHMW/PP/Nylon/ABS/PETG/PVC additions
            // bin into the closest available chemistry — these are
            // best-effort routing hints, not citation-backed mappings.
            // When no vendor LUT row exists (the common case for the
            // newer families) the lookup misses cleanly and the
            // formula fallback takes over, so the binning has
            // minimal practical impact today. TODO Phase E+: extend
            // `MaterialFamily` with per-family bins as LUT rows land.
            let lut_family = match family {
                crate::material::PlasticFamily::Acrylic
                | crate::material::PlasticFamily::Generic
                // ABS / PETG / PVC are amorphous rigid sheet plastics
                // — Acrylic is the LUT's closest "rigid amorphous" bin.
                | crate::material::PlasticFamily::Abs
                | crate::material::PlasticFamily::Petg
                | crate::material::PlasticFamily::RigidPvc => MaterialFamily::Acrylic,
                // UHMW-PE / PP are polyolefins like HDPE; HDPE is the
                // LUT's closest polyolefin bin.
                crate::material::PlasticFamily::Hdpe
                | crate::material::PlasticFamily::UhmwPe
                | crate::material::PlasticFamily::Polypropylene => MaterialFamily::Hdpe,
                // Nylon 6/6 routes to Delrin (the LUT's engineering-
                // thermoplastic bin — POM and PA are both crystalline
                // engineering polymers with comparable machining
                // behaviour).
                crate::material::PlasticFamily::Delrin
                | crate::material::PlasticFamily::Nylon66 => MaterialFamily::Delrin,
                crate::material::PlasticFamily::Polycarbonate => MaterialFamily::Polycarbonate,
            };
            // Canonical hardness from PlasticFamily::hardness().
            // PMMA reads in Rockwell M natively, ABS/PETG read in
            // Rockwell R — the LUT row's hardness scaling needs the
            // matching kind. When the family has no fetched hardness
            // (Generic), default to a Shore-D-equivalent 80 to keep
            // the lookup numeric without inventing a citation.
            let (hardness_kind, hardness_value) = match family.hardness() {
                Some(PlasticHardness::ShoreD(v)) => (HardnessKind::ShoreD, v),
                Some(PlasticHardness::RockwellM(v)) | Some(PlasticHardness::RockwellR(v)) => {
                    // The LUT currently models only Janka / Hb / ShoreD.
                    // Rockwell M (PMMA) and Rockwell R (ABS/PETG) are
                    // both surfaced under ShoreD as comparable-magnitude
                    // scalars until the LUT grows a Rockwell kind.
                    // TODO Phase 3+: plumb Rockwell HardnessKind through
                    // `vendor_lut.rs` (would need per-row Rockwell
                    // scale annotations on the vendor side too).
                    (HardnessKind::ShoreD, v)
                }
                None => (HardnessKind::ShoreD, 80.0),
            };
            (lut_family, hardness_kind, hardness_value)
        }
        Material::Aluminum { alloy } => (
            MaterialFamily::Aluminum,
            HardnessKind::Hb,
            alloy.brinell_hb(),
        ),
        Material::Fiberglass { .. } => {
            // Fiber-reinforced composites — own MaterialFamily +
            // material_category (3) in vendor_lookup. No fetched
            // hardness measurement on the wood / metal scale; emit
            // ShoreD as a structural placeholder so the lookup is
            // numerically defined (LUT row scoring against this
            // value contributes 0 since no Fiberglass rows currently
            // carry a hardness annotation).
            (MaterialFamily::Fiberglass, HardnessKind::ShoreD, 110.0)
        }
        Material::Foam { .. } => {
            // Foam has no LUT data — formula fallback regardless of the
            // values we return here. The triple is structural noise.
            (MaterialFamily::Softwood, HardnessKind::Janka, 200.0)
        }
        Material::Custom {
            feed_scale_factor, ..
        } => {
            // `Material::Custom` is the user-typed escape hatch — it
            // carries no true Janka reading. For LUT matching we need
            // *some* `(MaterialFamily, HardnessKind, value)` triple,
            // so we heuristically project `feed_scale_factor` onto a
            // wood-equivalent Janka by inverting the wood-class
            // `factor = (janka / 600)^0.4` formula.
            //
            // This is documented as a heuristic, not a physical
            // hardness — the alternative (refusing the lookup) would
            // silently break the vendor LUT fallback that every Custom
            // user depends on.
            //
            // S2-8 Option B note: `Material::wood_hardness_lbf()`
            // correctly returns `None` for `Custom`; this heuristic
            // Janka is local to the LUT matcher and does NOT round-trip
            // back through the material API.
            let janka = feed_scale_factor * 600.0;
            let family = if janka <= 800.0 {
                MaterialFamily::Softwood
            } else {
                MaterialFamily::Hardwood
            };
            (family, HardnessKind::Janka, janka)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::feeds::ToolGeometryHint;
    use crate::feeds::vendor_lut::ToolFamily;
    use crate::machine::MachineProfile;
    use crate::material::{PlasticFamily, SheetGoodKind, WoodSpecies};

    fn make_input<'a>(
        geom: ToolGeometryHint,
        material: &'a Material,
        machine: &'a MachineProfile,
        op: OperationFamily,
    ) -> FeedsInput<'a> {
        FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            shank_diameter: None,
            tool_geometry: geom,
            material,
            machine,
            operation: op,
            operation_kind: None,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: Default::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        }
    }

    #[test]
    fn test_flat_softwood_maps_correctly() {
        let mat = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(
            ToolGeometryHint::Flat,
            &mat,
            &mach,
            OperationFamily::Adaptive,
        );
        let query = to_lookup_query(&input).expect("no routing refusal in this fixture");
        assert_eq!(query.tool_family, ToolFamily::FlatEnd);
        assert_eq!(query.material_family, MaterialFamily::Softwood);
        assert_eq!(query.hardness_value, Some(600.0));
        assert_eq!(query.operation_family, LutOperationFamily::Adaptive);
    }

    #[test]
    fn test_ball_hardwood_maps_correctly() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(
            ToolGeometryHint::Ball,
            &mat,
            &mach,
            OperationFamily::Parallel,
        );
        let query = to_lookup_query(&input).expect("no routing refusal in this fixture");
        assert_eq!(query.tool_family, ToolFamily::BallNose);
        assert_eq!(query.material_family, MaterialFamily::Hardwood);
        assert_eq!(query.hardness_value, Some(1450.0));
    }

    #[test]
    fn test_acrylic_maps_to_canonical_pmma_hardness() {
        // Phase 1E consolidation: vendor_normalize now routes through
        // PlasticFamily::hardness() instead of an inline table. PMMA
        // is reported in Rockwell M (MakeItFrom citation) — the
        // hardness number is the literature value, not the historical
        // inline ShoreD 85. The LUT currently only carries ShoreD as
        // its scalar kind for plastics, so we surface the RockwellM
        // value via ShoreD — see TODO in material_to_lut to plumb a
        // RockwellM HardnessKind end-to-end.
        let mat = Material::Plastic {
            family: PlasticFamily::Acrylic,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(
            ToolGeometryHint::Flat,
            &mat,
            &mach,
            OperationFamily::Contour,
        );
        let query = to_lookup_query(&input).expect("no routing refusal in this fixture");
        assert_eq!(query.material_family, MaterialFamily::Acrylic);
        assert_eq!(query.hardness_kind, Some(HardnessKind::ShoreD));
        assert_eq!(query.hardness_value, Some(93.0));
    }

    #[test]
    fn test_mdf_maps_correctly() {
        let mat = Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(ToolGeometryHint::Flat, &mat, &mach, OperationFamily::Pocket);
        let query = to_lookup_query(&input).expect("no routing refusal in this fixture");
        assert_eq!(query.material_family, MaterialFamily::Mdf);
        assert_eq!(query.hardness_value, Some(1100.0));
    }
}
