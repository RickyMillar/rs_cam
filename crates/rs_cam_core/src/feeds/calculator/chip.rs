//! Step 2 of the calculator: the chip load, from the vendor row first and
//! the formula second.

use super::Chip;
use crate::compute::catalog::LutRoute;
use crate::feeds::sourced::Basis;
use crate::feeds::{
    ChiploadBounds, ChiploadSource, FeedsInput, FeedsWarning, chip_basis_of_row, geometry, support,
    vendor_lookup, vendor_lut, vendor_normalize,
};
use crate::material::{ConstantSource, FormulaConstant};

/// Step 2: the chip load. `effective_d` is the LUT-semantics engaged
/// diameter (`speed::engaged_diameter`).
pub(super) fn step2_chip_load(
    input: &FeedsInput,
    effective_d: f64,
    warnings: &mut Vec<FeedsWarning>,
) -> Chip {
    let machine = input.machine;
    let material = input.material;
    // --- Step 2: Chip load — vendor LUT first, formula fallback ---
    let feed_scale = material.feed_scale_factor();
    let cl = &machine.chip_load;
    // Formula chipload uses engaged diameter so the V-bit / tapered-ball
    // fallback path matches the LUT path's band semantics. For Flat /
    // Ball / Bull this collapses to nominal D.
    //
    // A drill cycle takes no multiplier (ruling B5, 2026-09-24). The repo
    // multiplier `DRILL_CHIPLOAD_MULTIPLIER` (2.5, unsourced; W6 audit
    // 2026-08-04 item R-11) is deleted. On a drill cell this formula is
    // now only the preview number of a refused cell (the explain modal, a
    // raw `calculate`): Suggest refuses every drill cell that no G6 claim
    // serves (`support::support_for_lookup`). A claimed cell reads its
    // chip from the vendor row (`extrapolation::drill`).
    let formula_chipload = cl.k0 * effective_d.powf(cl.p) * (1.0 / feed_scale).powf(cl.q);
    // P5 (K2): the formula chip names its three machine rows (F201).
    let formula_chip_basis = {
        let chip_load_claim = ConstantSource::Unsourced {
            claim: "\"Soft wood baseline from Shapeoko empirical data\" (reference/, not in the repo)",
        };
        let row = |key: &'static str, value: f64| {
            Basis::Rule(FormulaConstant {
                name: std::borrow::Cow::Borrowed(key),
                value,
                source: chip_load_claim,
            })
        };
        Basis::derived(
            "formula chip k0 x De^p x (1/H)^q",
            vec![
                row("machine.ChipLoadFormula.default.k0", cl.k0),
                row("machine.ChipLoadFormula.default.p", cl.p),
                row("machine.ChipLoadFormula.default.q", cl.q),
            ],
        )
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
    // A2 (point mode): the raw printed point of the recipe row, and of the
    // envelope row that the floor falls back to (the P1 sidecar above).
    // A point is never a band, so `RequireBoth` drops it from both bands;
    // these two carry it. The depth de-rate is applied below, with the band.
    let mut chipload_point_raw: Option<f64> = None;
    let mut floor_point_fallback_raw: Option<f64> = None;
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
    if matches!(row_lookup, support::RecipeRowLookup::NoRow)
        && input.tool_geometry.cutter_kind().lut_family() == vendor_lut::ToolFamily::FlatEnd
        && input
            .operation_kind
            .is_some_and(|k| k.capabilities().lut_route == LutRoute::FlatAsRoughPocket)
    {
        // P13 item 4 (law B): the tool has no (Pocket, Roughing) row of its
        // own material. The formula chip stays, and the warning says so (D7).
        warnings.push(FeedsWarning::FormulaChipNoPrintedRow {
            formula_chipload_mm: formula_chipload,
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
        chip_basis,
    ) = if let support::RecipeRowLookup::Row {
        lut,
        query,
        row: result,
    } = row_lookup
    {
        let result = *result;
        matched_lut_row = Some(result.clone());
        let row_chip_basis = chip_basis_of_row(&result);
        let size_refused = result.size_basis.is_refused();
        // Read the printed point before `observation_id` moves out of `result`.
        let printed_point = result.printed_chipload().point_mm();
        let observation_id = result.observation_id;
        // Capture the LUT-derived chipload band (post diameter
        // /hardness scaling) for Suggest v2 step 2's feed-up
        // recalibration loop. Only populated when the row
        // publishes both bounds — partial-band rows (one side
        // only) leave it None so the loop doesn't fire on an
        // ambiguous target.
        //
        // The band here is the printed row band, validated
        // (`geometry::validate_chipload_bounds`, S.8). P11b (L6): no depth
        // de-rate follows; the band is the printed band at every depth.
        let bounds = geometry::validate_chipload_bounds(
            result.chip_load_min_mm,
            result.chip_load_max_mm,
            geometry::ChiploadBoundPolicy::RequireBoth,
        )
        .and_then(geometry::ValidatedChiploadBand::into_pair)
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
        if size_refused {
            // Extrapolation P1 step 3: the G1 size claim refused this row
            // (`LookupResult::size_basis`). The row publishes no band and
            // the support arm is `Refuse`, so nothing of the row may ship:
            // the formula chipload, no vendor RPM, no band. This is not an
            // RPM-only row, so `VendorRowPublishesNoChipload` does not
            // apply. The row stays in `matched_lut_row` so that a reader
            // can see which row the size rule refused.
            (
                formula_chipload,
                None,
                None,
                None,
                None,
                ChiploadSource::FormulaFallback,
                None,
                formula_chip_basis,
            )
        } else if result.chip_load_mm > 0.0 {
            chipload_point_raw = printed_point;
            (
                result.chip_load_mm,
                result.rpm_nominal,
                result.rpm_max,
                result.rpm_min,
                Some(observation_id.clone()),
                ChiploadSource::VendorLut { observation_id },
                bounds,
                row_chip_basis,
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
            {
                if let Some((min, max)) = geometry::validate_chipload_bounds(
                    env.chip_load_min_mm,
                    env.chip_load_max_mm,
                    geometry::ChiploadBoundPolicy::RequireBoth,
                )
                .and_then(geometry::ValidatedChiploadBand::into_pair)
                {
                    floor_band_fallback_raw = Some(ChiploadBounds {
                        min_mm_per_tooth: min,
                        max_mm_per_tooth: max,
                    });
                    floor_band_row = Some(env.observation_id);
                } else if let Some(v) = env.printed_chipload().point_mm() {
                    // A2: the envelope row prints one value. The floor
                    // reads the point (`rubbing_floor_at`).
                    floor_point_fallback_raw = Some(v);
                    floor_band_row = Some(env.observation_id);
                }
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
                formula_chip_basis,
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
            formula_chip_basis,
        )
    };
    Chip {
        chip_load,
        basis: chip_basis,
        vendor_rpm,
        vendor_rpm_max,
        vendor_rpm_min,
        vendor_source,
        chipload_source,
        band: chipload_band_raw,
        point: chipload_point_raw,
        floor_band_fallback: floor_band_fallback_raw,
        floor_point_fallback: floor_point_fallback_raw,
        matched_lut_row,
        support,
        formula_chipload,
        feed_scale,
    }
}
