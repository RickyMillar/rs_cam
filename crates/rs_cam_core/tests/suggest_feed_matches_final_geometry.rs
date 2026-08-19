//! Sentry: **Suggest's commanded feed must be derived at the geometry
//! Suggest actually ships** (G-SUGGEST-STALE-GEOM, 2026-08-19).
//!
//! ## The defect this pins
//!
//! `feeds::calculate` bakes two geometry-dependent terms into the feed it
//! returns — radial × axial chip thinning (Step 5) and the depth tier
//! (Step 5a) — and both read the `ae` / `ap` the *calculator* chose.
//! `suggest::enforce_invariants` then rewrites those values freely:
//! `backoff_stepover_for_runtime` raises stepover for move-count sanity,
//! `pick_axial_envelope` / `clamp_dpp_to_rigidity` / `backoff_dpp_for_deflection`
//! lower DPP. Nothing reconciled the feed with the result. Suggest pass 8
//! was the last stage that touched the feed after the clamps, and its
//! retirement on 2026-08-13 (Checkpoint J-1) left the gap open.
//!
//! Measured motivating case, `wanaka200.toml` toolpath 8 ("7 3D Finish
//! (R1.5 drop_cutter)", Ø3.0 R1.5 tapered ball, 2F, White Oak, 19 000 rpm):
//!
//! ```text
//! ae the calculator used        0.09000   (operation_default_profile
//!                                          Parallel/Finish ae_factor 0.03 × Ø3 —
//!                                          NO operation sets FeedsHints::radial_width_mm)
//! ae the operation runs at      0.30375   (backoff_stepover_for_runtime)
//! radial chip thinning @ 0.09   2.2942    ← baked into the feed
//! radial chip thinning @ 0.304  1.3350    ← what the op actually needs
//! commanded advance             0.033154 mm/tooth = 1.613× the derated band max
//! ```
//!
//! The mirror case under-feeds: a rough whose DPP is clamped 9.0 → 4.2 mm
//! keeps `depth_tier = 0.75` computed at 9.0 mm, where 4.2 mm on a Ø6 tool
//! is tier 1.0 — 1.33× slow against the engine's own model.
//!
//! ## What is asserted, and why in this shape
//!
//! Per this repo's pin convention (see `wanaka_suggest_integration.rs`),
//! the primary assertion is a **mechanism**, not a magnitude:
//!
//!   implied target chipload = commanded advance ÷ (chip thinning × depth tier)
//!
//! must come out the same whether you evaluate it at the calculator's own
//! operating point or at the operation's final one. Everything else in the
//! calculator's feed expression (target chipload, RPM, flute count, the L/D
//! overhang derate, the power limit, the safety factor) is independent of
//! `ae` / `ap`, so it cancels from that ratio exactly. When the two disagree,
//! the feed is quoting a geometry the operation does not run.
//!
//! The second assertion is the consequence the operator sees: on a fixture
//! whose stepover is backed off, the commanded advance per tooth must not
//! sit **above** the matched row's derated band maximum. That is not a
//! generic law — chip thinning legitimately lifts the commanded advance past
//! the band on genuinely thin radial cuts, which is the separately-ledgered
//! `G-CHIPTHIN-HALFFIX` decision and is NOT in scope here. It is asserted on
//! this fixture because at its *final* geometry the lift does not reach the
//! band edge, so anything above it can only be stale-geometry residue.
//!
//! Both assertions read the shipped geometry helpers
//! (`feeds::effective_diameter`, `feeds::geometry::*`) rather than carrying
//! private copies — a second implementation of the chip-thinning diameter is
//! precisely what C3 retired in 2026-08.
//!
//! Fixture: `tests/fixtures/wanaka_2026-08-16_f530995a.toml`, the same dated
//! snapshot `wanaka_suggest_integration.rs` pins, for the same reason (a
//! play-file a human edits cannot distinguish "Suggest regressed" from "the
//! operator tried something").

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId};
use rs_cam_core::feeds::suggest::{SuggestWarning, SuggestedParams};
use rs_cam_core::feeds::{FeedsResult, ToolGeometryHint, effective_diameter, geometry};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::ProjectSession;

fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wanaka_2026-08-16_f530995a.toml")
}

/// The product of the two geometry-dependent terms `feeds::calculate` folds
/// into its feed: `clamp(radial_thinning × axial_thinning, 1, 4) × depth_tier`.
///
/// Mirrors the calculator's SHADOW POINT: chip thinning reads the effective
/// diameter at the commanded axial DOC, the depth tier reads the nominal one.
fn geometry_factor(
    geom: ToolGeometryHint,
    nominal_d_mm: f64,
    shank_d_mm: f64,
    ae_mm: f64,
    ap_mm: f64,
) -> f64 {
    let effective_d = effective_diameter(geom, nominal_d_mm, shank_d_mm, ap_mm);
    let rctf = geometry::radial_chip_thinning_factor(ae_mm, effective_d);
    let axial = match geom {
        ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
            geometry::axial_chip_thinning_factor_for_ball(nominal_d_mm, effective_d)
        }
        _ => 1.0,
    };
    (rctf * axial).clamp(1.0, 4.0) * geometry::depth_tier_multiplier(ap_mm, nominal_d_mm)
}

struct Case {
    name: String,
    tool: ToolConfig,
    suggested: SuggestedParams,
}

fn suggest_case(session: &ProjectSession, id: ToolpathId) -> Case {
    let tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| {
            panic!(
                "Toolpath {id} missing from the wanaka fixture — shape changed? ids: {:?}",
                session
                    .toolpath_configs()
                    .iter()
                    .map(|t| (t.id, t.name.as_str()))
                    .collect::<Vec<_>>()
            )
        });
    let tool = session
        .get_tool(ToolId(tc.tool_id))
        .unwrap_or_else(|| panic!("Tool {} missing for toolpath {id}", tc.tool_id))
        .clone();
    let profile = session
        .cutter_op_profile(tc)
        .unwrap_or_else(|| panic!("No cutter/op profile for toolpath {id}"));
    profile
        .feasibility
        .unwrap_or_else(|e| panic!("Suggest refused toolpath {id}: {e:?}"));
    Case {
        name: tc.name.clone(),
        tool,
        suggested: SuggestedParams {
            operation: profile
                .suggested_operation
                .unwrap_or_else(|| panic!("feasible combo without suggested operation, tp {id}")),
            feeds_result: profile
                .feeds
                .unwrap_or_else(|| panic!("feasible combo without feeds result, tp {id}")),
            warnings: profile.warnings,
            provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        },
    }
}

/// `(commanded advance per tooth, geometry factor, implied target chipload)`
/// at the operation's FINAL values.
fn final_operating_point(case: &Case) -> (f64, f64, f64) {
    let op = &case.suggested.operation;
    let flutes = f64::from(case.tool.flute_count.max(1));
    let rpm = f64::from(op.spindle_rpm().expect("Suggest writes the calculator RPM"));
    let advance = op.feed_rate() / (rpm * flutes);
    let geom = build_cutter(&case.tool).to_geometry_hint();
    let factor = geometry_factor(
        geom,
        case.tool.diameter,
        case.tool.shank_diameter,
        op.stepover().expect("fixture op carries a stepover"),
        // Surface-following ops (drop_cutter and friends) command no axial
        // step at all — `narrate_toolpath` says so in those words. Their
        // axial geometry therefore cannot go stale, so fall back to the
        // depth the calculator sized against: the depth-tier term is then
        // identical at both operating points and cancels from the ratio,
        // leaving the stepover as the only thing under test.
        op.depth_per_pass()
            .unwrap_or(case.suggested.feeds_result.axial_depth_mm),
    );
    (advance, factor, advance / factor)
}

/// The same triple at the point the CALCULATOR chose, straight off its own
/// result — i.e. the operating point the feed was legitimately derived at.
fn calculator_operating_point(case: &Case) -> (f64, f64, f64) {
    let r: &FeedsResult = &case.suggested.feeds_result;
    let flutes = f64::from(case.tool.flute_count.max(1));
    let advance = r.feed_rate_mm_min / (r.rpm * flutes);
    let geom = build_cutter(&case.tool).to_geometry_hint();
    let factor = geometry_factor(
        geom,
        case.tool.diameter,
        case.tool.shank_diameter,
        r.radial_width_mm,
        r.axial_depth_mm,
    );
    (advance, factor, advance / factor)
}

fn dump(case: &Case) {
    let op = &case.suggested.operation;
    let r = &case.suggested.feeds_result;
    let (adv_f, fac_f, tgt_f) = final_operating_point(case);
    let (adv_c, fac_c, tgt_c) = calculator_operating_point(case);
    println!("── {} ({})", case.name, case.tool.name);
    println!(
        "   calculator : ae={:.5} ap={:.4} feed={:.3} rpm={:.0} factor={fac_c:.5} \
         advance={adv_c:.6} implied_target={tgt_c:.6}",
        r.radial_width_mm, r.axial_depth_mm, r.feed_rate_mm_min, r.rpm
    );
    println!(
        "   shipped    : ae={:?} ap={:?} feed={:.3} rpm={:?} factor={fac_f:.5} \
         advance={adv_f:.6} implied_target={tgt_f:.6}",
        op.stepover(),
        op.depth_per_pass(),
        op.feed_rate(),
        op.spindle_rpm()
    );
    println!("   band       : {:?}", r.chipload_bounds);
    for w in &case.suggested.warnings {
        println!("   warn       : {w:?}");
    }
}

/// The load-bearing assertion. The implied target chipload — commanded
/// advance divided out by the geometry terms — must agree between the two
/// operating points, because every other factor in the feed expression is
/// geometry-independent and cancels.
///
/// The one licensed exception is the rubbing-floor clamp, which deliberately
/// overrides the derates; when it fires the advance must sit exactly on the
/// floor it reports instead.
fn assert_feed_consistent_with_final_geometry(case: &Case) {
    let (adv_f, _, tgt_f) = final_operating_point(case);
    let (_, _, tgt_c) = calculator_operating_point(case);

    if let Some((floor, _)) = case.suggested.warnings.iter().find_map(|w| match w {
        SuggestWarning::FeedClampedToChiploadFloor {
            floor_mm_per_tooth,
            requested_mm_per_tooth,
            ..
        } => Some((*floor_mm_per_tooth, *requested_mm_per_tooth)),
        _ => None,
    }) {
        assert!(
            (adv_f - floor).abs() <= floor * 1e-6,
            "{}: the rubbing-floor clamp fired, so the commanded advance must sit ON the \
             floor it reports — floor {floor:.8}, commanded {adv_f:.8}",
            case.name
        );
        return;
    }

    let rel = (tgt_f - tgt_c).abs() / tgt_c.max(f64::MIN_POSITIVE);
    assert!(
        rel <= 1e-6,
        "{}: the shipped feed is derived at a geometry the operation does not run.\n  \
         implied target chipload at the CALCULATOR's ae/ap : {tgt_c:.8} mm/tooth\n  \
         implied target chipload at the SHIPPED   ae/ap    : {tgt_f:.8} mm/tooth\n  \
         relative disagreement {rel:.4} (tolerance 1e-6)\n  \
         Every non-geometric term (target chipload, rpm, flutes, L/D derate, power limit, \
         safety factor) cancels from this ratio, so a disagreement means the chip-thinning \
         and/or depth-tier terms were computed at a stepover/DPP a later invariant pass \
         then overwrote.",
        case.name
    );
}

#[test]
fn stepover_backoff_does_not_leave_a_stale_chip_thinning_lift_in_the_feed() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "wanaka snapshot fixture missing at {} — this sentry needs its dated snapshot on disk",
        path.display()
    );
    let session = ProjectSession::load(&path).expect("load wanaka fixture");

    // Toolpath 11, "3D Finish 6" (DropCutter on a tapered ball): its
    // scallop-height target resolves to a ~0.03 mm stepover, which
    // `backoff_stepover_for_runtime` raises by ~7.6× for move-count sanity.
    // That raise is exactly the mutation the calculator's chip-thinning lift
    // was computed before.
    let case = suggest_case(&session, ToolpathId(11));
    dump(&case);

    let raised = case
        .suggested
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::StepoverRaisedForRuntime {
                requested_mm,
                raised_mm,
                ..
            } => Some((*requested_mm, *raised_mm)),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "{}: this sentry requires the runtime stepover back-off to fire — without a \
                 mutated stepover there is no stale geometry to catch. Warnings: {:?}",
                case.name, case.suggested.warnings
            )
        });
    assert!(
        raised.1 > raised.0,
        "{}: stepover back-off must raise, got {} → {}",
        case.name,
        raised.0,
        raised.1
    );

    assert_feed_consistent_with_final_geometry(&case);

    // Consequence the operator sees. See the module header for why this is
    // asserted on this fixture rather than stated as a general law.
    let (advance, _, _) = final_operating_point(&case);
    let band = case
        .suggested
        .feeds_result
        .chipload_bounds
        .expect("wanaka's tapered-ball finish matches a vendor LUT row");
    assert!(
        advance <= band.max_mm_per_tooth * (1.0 + 1e-9),
        "{}: commanded advance {advance:.8} mm/tooth exceeds the derated band maximum \
         {:.8} by {:.3}× — at the operation's FINAL stepover the chip-thinning lift does \
         not reach the band edge, so anything above it is residue from the pre-back-off \
         stepover.",
        case.name,
        band.max_mm_per_tooth,
        advance / band.max_mm_per_tooth
    );
}

#[test]
fn dpp_clamp_does_not_leave_a_stale_depth_tier_derate_in_the_feed() {
    let path = wanaka_project_path();
    let session = ProjectSession::load(&path).expect("load wanaka fixture");

    // Toolpath 4, "Back Rough" (Adaptive3d, Ø6 carbide endmill): the axial
    // envelope clamps the calculator's 9.0 mm DPP to the matched row's
    // 4.2 mm ceiling. 9.0 mm on Ø6 is depth tier 0.75; 4.2 mm is tier 1.0.
    // The mirror of the finish case — this one is under-fed by 1.333×
    // against the engine's own model.
    let case = suggest_case(&session, ToolpathId(4));
    dump(&case);

    let clamped = case
        .suggested
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::AxialDocClampedByEnvelope {
                commanded_mm,
                clamped_mm,
                ..
            } => Some((*commanded_mm, *clamped_mm)),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "{}: this sentry requires the axial envelope clamp to fire. Warnings: {:?}",
                case.name, case.suggested.warnings
            )
        });
    assert!(
        clamped.1 < clamped.0,
        "{}: envelope must clamp DPP down, got {} → {}",
        case.name,
        clamped.0,
        clamped.1
    );
    // Direction pin, not a magnitude: crossing the 1×D depth-tier boundary
    // downward removes a derate, so the reconciled feed must be strictly
    // faster than the one the calculator froze at 9.0 mm.
    assert!(
        geometry::depth_tier_multiplier(clamped.0, case.tool.diameter)
            < geometry::depth_tier_multiplier(clamped.1, case.tool.diameter),
        "{}: fixture no longer crosses a depth-tier boundary ({} → {} mm on Ø{}) — \
         re-pick the case or this test proves nothing",
        case.name,
        clamped.0,
        clamped.1,
        case.tool.diameter
    );

    assert_feed_consistent_with_final_geometry(&case);
}
