//! C3 — parity sentry for the tapered-ball width models.
//!
//! Three implementations of "how wide is a tapered ball at depth h" used to
//! coexist in this crate. Two remain, and they are the same math on two
//! different carriers:
//!
//! 1. [`rs_cam_core::tool::MillingCutter::width_at_height`] on
//!    `TaperedBallEndmill` — the CANONICAL profile: a hemisphere of radius
//!    `R` blended TANGENTIALLY into a cone of half-angle `alpha`. This is
//!    what the dexel simulator, `reach.rs` and the pushcutter engage.
//! 2. [`rs_cam_core::feeds::ToolGeometryHint::engaged_diameter_at_doc`] —
//!    the hint-level twin of (1), which exists because Suggest's call sites
//!    carry scalar shape params rather than a `&dyn MillingCutter`. Kept
//!    honest against `MillingCutter::lookup_diameter_at` by
//!    `feeds::tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`;
//!    re-anchored here so this file states the whole triangle.
//!
//! # What was retired, and what it cost
//!
//! `feeds::geometry::tapered_ball_effective_diameter` was a STRAIGHT CONE
//! rooted at the tip radius, `2*(tip_r + ap*tan(alpha))`, with no tangency
//! blend, clamped to the nominal diameter. Measured against (1) over the
//! sweep in [`CASES`] x [`DOCS`] on 2026-08-02, at the binding every
//! production call site actually produced (`tip_r == nominal_d / 2`):
//!
//! | | divergence | where |
//! |---|---|---|
//! | worst overstatement | **+290.6%** | Ø3 tip / 15° taper @ 0.05 mm DOC |
//! | worst understatement | **−44.1%** | Ø0.5 tip / 3° taper @ 4.00 mm DOC |
//! | pure missing-tangency error, clamp removed | **+8.75%** | Ø1 tip / 5.26° @ 0.5 mm DOC |
//!
//! The sign flips at the tangency height: below it the straight cone starts
//! at the full tip diameter where the true profile starts at zero width, so
//! it overstates; above it the `.clamp(0.01, nominal_d)` pins the cone at
//! the tip diameter while the real shoulder keeps growing toward the shank,
//! so it understates. The backlog's "~5% off at 0.5 mm DOC" described only
//! the narrow neighbourhood where the two error sources cancel.
//!
//! Worse than any of that, its growth term was DEAD. `feeds::effective_
//! diameter` was its only caller, reached only through
//! `feeds::suggest::feeds_input_for_operation`, which sets `tool_diameter =
//! tool.diameter` and derives the hint from the SAME tool — so `tip_r ==
//! nominal_d / 2` always, and `(nominal_d + 2*ap*tan(alpha)).clamp(0.01,
//! nominal_d)` is the constant `nominal_d` for every `ap >= 0`. A tapered
//! ball was fed as if it engaged its full tip diameter at any depth, however
//! shallow, while a plain ball of the same tip size got the exact contact
//! circle.
//!
//! # What retiring it changed
//!
//! Suggest's recommended feed for shallow tapered-ball finishing rises by
//! **+8% to +26%**; deeper than the tangency height, nothing moves. The full
//! before/after table is pinned in [`the_c3_feed_delta_is_pinned`]. Nothing
//! but Suggest reads this path, so no project or generated toolpath changes
//! on its own.
//!
//! Read with `planning/review_2026-07-29/ANTIPATTERNS_BACKLOG.md` P3.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, calculate,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};

/// Realistic tapered-ball geometries: (ball tip Ø, taper half-angle°, shaft Ø).
///
/// Row 0 is the wanaka finishing tool (Ø1 tip). Rows 1–3 walk the taper
/// angle and the tip size across the range the tool library ships.
const CASES: &[(f64, f64, f64)] = &[
    (1.0, 5.26, 6.35),
    (0.5, 3.0, 3.175),
    (2.0, 10.0, 6.35),
    (3.0, 15.0, 12.0),
];

/// The DOC ladder, mm. 0.5 mm is the depth the backlog note quotes.
const DOCS: &[f64] = &[0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 4.0];

/// (1) — the canonical profile, as a DIAMETER.
fn canonical_diameter_at(case: (f64, f64, f64), doc: f64) -> f64 {
    let (ball_d, half_angle, shaft_d) = case;
    let cutter = TaperedBallEndmill::new(ball_d, half_angle, shaft_d, 30.0);
    2.0 * cutter.width_at_height(doc)
}

fn hint_for(case: (f64, f64, f64)) -> ToolGeometryHint {
    ToolGeometryHint::TaperedBall {
        tip_radius: case.0 / 2.0,
        taper_angle_deg: case.1,
    }
}

/// The hint-level model (2) reproduces the canonical profile (1) exactly.
/// This is the anchor of the whole file: there is now ONE tapered-ball width
/// answer, reachable from either carrier.
#[test]
fn the_hint_model_reproduces_the_canonical_profile_exactly() {
    for &case in CASES {
        let (ball_d, _, shaft_d) = case;
        for &doc in DOCS {
            let canonical = canonical_diameter_at(case, doc).min(shaft_d);
            let hinted = hint_for(case).engaged_diameter_at_doc(doc, ball_d, shaft_d);
            assert!(
                (canonical - hinted).abs() < 1e-12,
                "hint vs canonical diverged: case={case:?} doc={doc} \
                 canonical={canonical} hint={hinted}"
            );
        }
    }
}

/// The retired straight cone's defining property was that it did NOT vary
/// with depth at the production binding. Whatever the feeds path reports for
/// a tapered ball must now vary — this is the regression guard against
/// reintroducing a constant under a different name.
///
/// `effective_diameter` is private, so this drives the public
/// `FeedsResult::effective_diameter_mm`, which is what every downstream
/// consumer (chipload-band DOC derating, the Suggest axial-DOC envelope
/// pass) actually reads.
#[test]
fn the_published_effective_diameter_varies_with_depth_for_a_tapered_ball() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let previous = &mut 0.0_f64;

    for &doc in &[0.05_f64, 0.1, 0.25, 0.5] {
        let result = calculate(&tapered_finish_input(&material, &machine, doc));
        let canonical = canonical_diameter_at(CASES[0], result.axial_depth_mm).min(6.35);
        assert!(
            (result.effective_diameter_mm - canonical).abs() < 1e-9,
            "published effective_diameter_mm at doc={doc} (commanded \
             {commanded}) is {published}, canonical profile says {canonical}",
            commanded = result.axial_depth_mm,
            published = result.effective_diameter_mm,
        );
        assert!(
            result.effective_diameter_mm > *previous,
            "effective_diameter_mm must grow with DOC: doc={doc} gave {} \
             after {previous}",
            result.effective_diameter_mm
        );
        *previous = result.effective_diameter_mm;
    }
}

fn tapered_finish_input<'a>(
    material: &'a Material,
    machine: &'a MachineProfile,
    doc: f64,
) -> FeedsInput<'a> {
    FeedsInput {
        tool_diameter: 1.0,
        flute_count: 2,
        flute_length: 12.0,
        shank_diameter: Some(6.35),
        tool_geometry: hint_for(CASES[0]),
        material,
        machine,
        operation: OperationFamily::Scallop,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(doc),
        radial_width_mm: Some(0.1),
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::default(),
    }
}

/// A tapered ball's tip IS a ball. Below the tangency height the two must
/// therefore report the SAME contact circle for the same tip at the same
/// depth — same physical geometry, same material engaged. Before C3 they did
/// not, because only one of them measured it: the tapered arm reported a
/// flat `1.000000` at every depth against the ball's exact chord.
///
/// The FEED still differs between them, and that is a SEPARATE, pre-existing
/// policy this wave deliberately did not touch:
/// [`ToolGeometryHint::engaged_diameter_at_doc`] selects the vendor-LUT
/// chipload row at the ENGAGED diameter for tapered/V geometries but at
/// NOMINAL for flat/ball/bull, so the two tools legitimately land on
/// different chipload rows. Measured here so the residual gap is attributed
/// rather than assumed to be the same defect.
#[test]
fn the_tapered_ball_feed_recipe_matches_its_ball_twin_below_tangency() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    // Tangency height for the Ø1 / 5.26° tip: R*(1 - sin(alpha)) = 0.4542 mm.
    // Every DOC below it engages ball geometry alone.
    for &doc in &[0.05_f64, 0.1, 0.25, 0.4] {
        let tapered = calculate(&tapered_finish_input(&material, &machine, doc));
        let ball = calculate(&FeedsInput {
            tool_geometry: ToolGeometryHint::Ball,
            ..tapered_finish_input(&material, &machine, doc)
        });
        assert!(
            (tapered.effective_diameter_mm - ball.effective_diameter_mm).abs() < 1e-9,
            "below tangency a tapered ball engages exactly its ball twin: \
             doc={doc} tapered={} ball={}",
            tapered.effective_diameter_mm,
            ball.effective_diameter_mm
        );
        eprintln!(
            "doc={doc:.2} tapered eff_d={:.6} feed={:.2} | ball eff_d={:.6} feed={:.2}",
            tapered.effective_diameter_mm,
            tapered.feed_rate_mm_min,
            ball.effective_diameter_mm,
            ball.feed_rate_mm_min
        );
    }
}

/// **THE BEHAVIOURAL DELTA OF C3.** Suggest's recommended feed for a
/// tapered-ball finishing pass, before and after retiring the straight cone.
/// Ø1 tip / 5.26° taper / Ø6.35 shank, GenericSoftwood, Shapeoko VFD
/// profile, scallop finish at 0.1 mm WOC. Measured 2026-08-02.
///
/// | DOC (mm) | eff_d before → after | feed before → after | Δ feed |
/// |---|---|---|---|
/// | 0.05 | 1.000000 → 0.435890 | 1200.00 → 1515.07 | **+26.3%** |
/// | 0.10 | 1.000000 → 0.600000 | 1200.00 → 1509.04 | **+25.8%** |
/// | 0.25 | 1.000000 → 0.866025 | 1406.97 → 1525.05 | **+8.4%** |
/// | 0.50 | 1.000000 → 1.004229 | 1539.96 → 1542.85 | +0.2% |
///
/// **The feed column of that table is now history.** On 2026-08-19
/// G-CHIPTHIN-HALFFIX deleted the chip-thinning multiplication from the feed,
/// and every one of those deltas was mediated by it — the corrected effective
/// diameter fed `axial_chip_thinning_factor_for_ball` and nothing else that
/// reaches a feed. All four rows have returned to the pre-C3 column. The
/// effective-diameter column, which is what C3 was actually about, is
/// unchanged and still asserted.
///
/// The change is a feed INCREASE on shallow tapered-ball finishing — the
/// direction that wants a human's eyes, so it is stated here and in the wave
/// log rather than buried in a diff. Its scope is bounded: this path is
/// reached only through Suggest, which is a button, so no existing project
/// or generated toolpath moves on its own.
///
/// It is not a tuning choice. The old number was the SAME at every depth
/// because the model collapsed to a constant, so there is no "previous
/// calibration" being overridden — only an inconsistency between a tapered
/// ball and its own ball twin, resolved in favour of the arm that was
/// already exact and already sentried against the cutter trait.
#[test]
fn the_c3_feed_delta_is_pinned() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    // (doc, eff_d before C3, feed before C3, eff_d after, feed NOW)
    //
    // **The feed column collapsed back on 2026-08-19 (G-CHIPTHIN-HALFFIX).**
    // C3's feed delta was mediated ENTIRELY by chip thinning: the corrected
    // effective diameter fed `axial_chip_thinning_factor_for_ball`, and that
    // factor multiplied the feed. With the multiplication deleted, effective
    // diameter no longer reaches the feed at all, so every row returns to its
    // pre-C3 value — 1200.00 / 1200.00 / 1406.97 / 1539.96, the third column.
    //
    // The four rows do not merely return to the pre-C3 column, they all
    // COLLAPSE ONTO 1200.00, and that number is not a coincidence: it is the
    // chip-formation floor itself, `RUBBING_FLOOR_MM_TOOTH × rpm × flutes`
    // = 0.025 × 24 000 × 2. Pre-C3 the two shallowest rows were already
    // sitting on that floor (which is why both read 1200.00 in the third
    // column); with the chip-thinning multiplier gone the two deeper rows fall
    // onto it as well. So this fixture no longer measures a feed model at all
    // — every row is the floor — and that is worth knowing about a sub-Ø2
    // tapered ball in softwood: Step 9b is the only thing setting its feed.
    //
    // **C3 itself is NOT reverted and is not in question.** The
    // `effective_diameter_mm` column is untouched and still asserted below —
    // that was C3's actual subject, and it still feeds the chipload band's DOC
    // derate, the cutter-trait parity sentries and the reported geometry. What
    // this test can no longer pin is a feed *delta*, because there is none.
    let rows: &[(f64, f64, f64, f64, f64)] = &[
        (0.05, 1.0, 1200.00, 0.435890, 1200.00),
        (0.10, 1.0, 1200.00, 0.600000, 1200.00),
        (0.25, 1.0, 1406.97, 0.866025, 1200.00),
        (0.50, 1.0, 1539.96, 1.004229, 1200.00),
    ];

    for &(doc, eff_before, feed_before, eff_after, feed_after) in rows {
        let r = calculate(&tapered_finish_input(&material, &machine, doc));
        assert!(
            (r.effective_diameter_mm - eff_after).abs() < 5e-6,
            "doc={doc}: effective_diameter_mm {} left its pinned post-C3 \
             value {eff_after} (pre-C3 it was the constant {eff_before})",
            r.effective_diameter_mm
        );
        assert!(
            (r.feed_rate_mm_min - feed_after).abs() < 0.01,
            "doc={doc}: feed {} left its pinned value {feed_after}. Since \
             G-CHIPTHIN-HALFFIX (2026-08-19) that value is also the pre-C3 one \
             ({feed_before}) — effective diameter no longer reaches the feed, so if \
             these have diverged again, something is multiplying the feed by a \
             geometry term. See the Step 5 note in feeds/mod.rs.",
            r.feed_rate_mm_min
        );
    }
}

/// Above the tangency height the cone shoulder takes over and the two
/// legitimately part company: the tapered tool keeps widening toward its
/// shank while the ball saturates at nominal. Pinned so "they agree
/// everywhere" is never mistaken for the contract.
#[test]
fn above_tangency_the_taper_outgrows_its_ball_twin() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let tapered = calculate(&tapered_finish_input(&material, &machine, 3.0));
    let ball = calculate(&FeedsInput {
        tool_geometry: ToolGeometryHint::Ball,
        ..tapered_finish_input(&material, &machine, 3.0)
    });
    assert!(
        tapered.effective_diameter_mm > ball.effective_diameter_mm + 1e-9,
        "tapered {} should outgrow ball {} at 3 mm DOC",
        tapered.effective_diameter_mm,
        ball.effective_diameter_mm
    );
    assert!(
        tapered.effective_diameter_mm <= 6.35 + 1e-9,
        "…but never past the shank: {}",
        tapered.effective_diameter_mm
    );
}
