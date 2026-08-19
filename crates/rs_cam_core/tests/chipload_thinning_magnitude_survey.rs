//! Survey: **how much does Suggest multiply the vendor chipload number, and
//! what happens if it stops?** (G-CHIPTHIN-HALFFIX, 2026-08-19)
//!
//! ## Why this exists
//!
//! The 2026-08-06 literature wave established from primary sources that every
//! vendor chipload column in the shipped LUT is a **linear advance per tooth**
//! — `Chip Load = Feed Rate / (RPM × flutes)`, verbatim, one chart numerically
//! self-verifying — and it **deleted** the gate-side chip-thinning
//! normalisation rather than inverting it.
//!
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` §2.3 / N-8
//! records the finding that decides the direction, and records it as a
//! deliberate negative result: **no wood source in the shipped LUT publishes a
//! radial-engagement condition for its chipload column.** Six wood charts were
//! read in full (Onsrud Hard/Soft Wood, Freud solid carbide, Amana ZrN 2D/3D
//! Carving, Amana Aluminum O-Flute, Amana Spektra Plastic O-Flute); all six
//! state an *axial* condition (1 × D) and an axial derate table, and none
//! mentions stepover, width of cut, radial engagement or `ae`. Only the two
//! **metal** families (Garr, Helical) state a radial condition, and both state
//! one at or above 0.5 D.
//!
//! **Until 2026-08-19 that correction had landed on the gate and nowhere
//! else.** `feeds/mod.rs` Step 5 multiplied the feed by
//! `clamp(radial × axial thinning, 1, 4)`, and `feed_modulation.rs` divided its
//! target by `sqrt(woc)` — a different formula making the same claim, and an
//! unbounded one, reaching 31.6× at its engagement floor. So Suggest could
//! command several times the vendor number by design while the post-sim gate
//! judged that same number unmultiplied, and the two were shown side by side on
//! operator surfaces. Both sites are now deleted.
//!
//! ## What this file is
//!
//! It began as an **instrument**: it measured the magnitude of the
//! multiplication across the shipped operation × tool × material surface and
//! printed the table the operator ruled on. It deliberately did not assert the
//! multiplication was wrong, because that was their call and it moved every
//! feed in the product.
//!
//! **The ruling came in on 2026-08-19: delete, and let the existing Step-9b
//! rubbing floor catch the fall.** The seed-target half of the question
//! (band midpoint vs band maximum) was explicitly NOT bundled with it and
//! remains open. So this file is now a **sentry as well**: it still prints the
//! table, and it additionally pins that the multiplier does not reach the
//! feed. The measured table is kept because the decision rests on it — a
//! future reader asking "why is chip thinning not applied here?" should find
//! the numbers, not just the conclusion.
//!
//! Run it with:
//!
//! ```text
//! cargo test -p rs_cam_core --test chipload_thinning_magnitude_survey -- --nocapture
//! ```
//!
//! ## Reading the "if re-applied" column
//!
//! Chip thinning WAS a pure multiplier on `raw_feed` at Step 5, so the feed the
//! engine used to emit is exactly `feed × observed_combined_chip_thinning` —
//! **provided no later clamp bound.** Three can: the power ceiling (Step 6), the machine
//! cutting-feed clamp (Step 7) and the rubbing-floor clamp (Step 9b). Any row
//! where one of those fired is marked `clamped` and its reconstruction is an
//! upper bound rather than an equality. That distinction is the whole reason
//! this is measured rather than divided on a napkin.
//!
//! ## Note on staleness
//!
//! The magnitudes recorded in `planning/airrun_2026-08-19/RUN_LOG.md`
//! (1.25× / 1.67× / 3.82× / 4.00×) were measured **before** Suggest pass 9
//! (G-SUGGEST-NOCLAMP, `a1bb964b`) landed. Pass 9 re-derives the thinning term
//! at the stepover the operation actually runs instead of the one the
//! calculator was handed, which on a backed-off finish pass cut the factor
//! from 3.8236 to 1.7171 on its own. Any decision about this defect must be
//! taken against the numbers below, not against those.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, suggest_params,
};
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsWarning, SpindleStrategy, WorkholdingRigidity};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

/// One measured operating point.
struct Point {
    op: OperationType,
    tool_kind: ToolType,
    diameter: f64,
    flutes: u32,
    material: String,
    /// `observed_combined_chip_thinning` — `clamp(radial × axial, 1, 4)`.
    /// Measured and reported by the engine; **not** applied to the feed.
    thinning: f64,
    radial: f64,
    axial: f64,
    /// Commanded advance per tooth as shipped (mm/tooth).
    advance: f64,
    /// Derated vendor band, when the match published one.
    band: Option<(f64, f64)>,
    /// A later clamp bound, so the reconstruction is an upper bound only.
    clamped: bool,
    /// The full derate breakdown, kept so the sentry below can recompose the
    /// APPLIED multipliers and check nothing else got into the feed.
    derates: Option<rs_cam_core::feeds::FeedsDerates>,
    /// `rpm × flutes` — the divisor that turns a feed into an advance per
    /// tooth. Kept so the sentry can express the apply path's 1 mm/min feed
    /// rounding as a tolerance in advance units.
    divisor: f64,
    /// The advance per tooth at the CALCULATOR's own feed, before the invariant
    /// passes ran.
    ///
    /// The mechanism sentry compares against this rather than against
    /// `advance`, and the distinction is load-bearing: `FeedsDerates` records
    /// what `feeds::calculate` composed, and Suggest pass 9 may legitimately
    /// rescale the feed afterwards when an invariant pass moves the DPP across
    /// a depth-tier boundary. Checking the shipped feed against the
    /// calculator's derates would flag that rescale as "an unexplained
    /// multiplier" — measured at exactly 4/3 on Adaptive3d Ø1.5 in MDF, which
    /// is a tier crossing, not a thinning term.
    calculator_advance: f64,
}

impl Point {
    /// **The direction of this comparison inverted on 2026-08-19.** Before the
    /// deletion, `advance` included the multiplier and this returned the
    /// counterfactual without it. Now `advance` is the shipped, unmultiplied
    /// value and this reconstructs what the engine USED to command — the
    /// multiplier is still measured, it is just no longer applied.
    ///
    /// Exact where no later clamp bound. Three could: the power ceiling
    /// (Step 6), the machine cutting-feed clamp (Step 7) and the rubbing-floor
    /// clamp (Step 9b). Rows where one fired are marked `clamped`, and for
    /// those this is an upper bound rather than an equality.
    fn if_thinning_were_reapplied(&self) -> f64 {
        self.advance * self.thinning
    }

    /// `Inside` / `Over` / `Under` the derated vendor band.
    fn placement(&self, advance: f64) -> Option<&'static str> {
        self.band.map(|(min, max)| {
            if advance > max * (1.0 + 1e-9) {
                "over"
            } else if advance < min * (1.0 - 1e-9) {
                "under"
            } else {
                "inside"
            }
        })
    }
    fn over_band(&self, advance: f64) -> Option<f64> {
        self.band
            .filter(|(_, max)| *max > 0.0)
            .map(|(_, max)| advance / max)
    }
    fn under_band(&self, advance: f64) -> Option<f64> {
        self.band
            .filter(|(min, _)| *min > 0.0)
            .map(|(min, _)| advance / min)
    }
}

fn tool_of(kind: ToolType, diameter: f64, flutes: u32) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = flutes;
    t.cutting_length = (diameter * 3.0).max(12.0);
    t.shank_diameter = diameter.max(3.0);
    t.shaft_diameter = diameter.max(3.0);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = diameter * 0.15;
        t.corner_radius_mm = diameter * 0.15;
    }
    if matches!(kind, ToolType::TaperedBallNose) {
        // Ø-at-tip tools: the nominal diameter IS the tip diameter, and the
        // shank is what the taper opens out to — so the shank must always be
        // the larger of the two. `tool/tapered_ball.rs:52` PANICS rather than
        // refusing when it is not, which is why this is computed rather than
        // pinned at 6.0.
        t.taper_half_angle = 7.0;
        t.shank_diameter = (diameter + 3.0).max(6.0);
        t.shaft_diameter = t.shank_diameter;
    }
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
    }
    t
}

fn stock_ctx() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    }
}

fn materials() -> Vec<(String, Material)> {
    vec![
        (
            "softwood/pine".to_owned(),
            Material::SolidWood {
                species: WoodSpecies::RadiataPine,
            },
        ),
        (
            "oak".to_owned(),
            Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
        ),
        (
            "hardmaple".to_owned(),
            Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
        ),
        (
            "ipe".to_owned(),
            Material::SolidWood {
                species: WoodSpecies::Ipe,
            },
        ),
        (
            "plywood".to_owned(),
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
        ),
        (
            "mdf".to_owned(),
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
    ]
}

fn sweep() -> Vec<Point> {
    let machine = MachineProfile::default();
    let stock = stock_ctx();
    let mut out = Vec::new();

    for &op in OperationType::ALL {
        for &kind in ToolType::ALL {
            for &(diameter, flutes) in &[(1.5_f64, 2_u32), (3.0, 2), (6.0, 2), (6.0, 3), (12.0, 3)]
            {
                let tool = tool_of(kind, diameter, flutes);
                for (label, material) in materials() {
                    let Ok(s) = suggest_params(SuggestParamsInput {
                        op_type: op,
                        tool: &tool,
                        machine: &machine,
                        material: &material,
                        workholding: WorkholdingRigidity::Medium,
                        lut: &EMBEDDED_LUT,
                        stock_ctx: &stock,
                        spindle_strategy: SpindleStrategy::default(),
                        context: SuggestContext::default(),
                    }) else {
                        // Refused pairings (scallop on a flat tool, and friends)
                        // are not survey points — the engine declined to run
                        // them at all.
                        continue;
                    };

                    let r = &s.feeds_result;
                    let rpm = s.operation.spindle_rpm().map_or(r.rpm, f64::from);
                    let divisor = rpm * f64::from(tool.flute_count.max(1));
                    if !(divisor.is_finite() && divisor > 0.0) {
                        continue;
                    }
                    let advance = s.operation.feed_rate() / divisor;
                    if !(advance.is_finite() && advance > 0.0) {
                        continue;
                    }
                    let clamped = r.warnings.iter().any(|w| {
                        matches!(
                            w,
                            FeedsWarning::PowerLimited { .. }
                                | FeedsWarning::FeedRateClamped { .. }
                                | FeedsWarning::ChiploadClampedToFloor { .. }
                        )
                    });
                    out.push(Point {
                        op,
                        tool_kind: kind,
                        diameter,
                        flutes,
                        material: label,
                        thinning: r.derates.observed_combined_chip_thinning,
                        radial: r.derates.observed_radial_chip_thinning,
                        axial: r.derates.observed_axial_chip_thinning,
                        advance,
                        band: r
                            .chipload_bounds
                            .map(|b| (b.min_mm_per_tooth, b.max_mm_per_tooth)),
                        clamped,
                        derates: Some(r.derates.clone()),
                        divisor,
                        calculator_advance: r.feed_rate_mm_min
                            / (r.rpm * f64::from(tool.flute_count.max(1))),
                    });
                }
            }
        }
    }
    out
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        100.0 * n as f64 / d as f64
    }
}

#[test]
fn chip_thinning_is_measured_but_not_applied_to_the_feed() {
    let points = sweep();
    assert!(
        points.len() > 200,
        "the survey swept only {} feasible points — the grid or the feasibility \
         rules changed and this instrument is no longer measuring the surface it \
         claims to",
        points.len()
    );

    let multiplied: Vec<&Point> = points.iter().filter(|p| p.thinning > 1.0 + 1e-9).collect();
    let banded: Vec<&Point> = points.iter().filter(|p| p.band.is_some()).collect();

    println!("\n=== G-CHIPTHIN-HALFFIX — magnitude survey (post pass 9) ===");
    println!(
        "swept {} feasible operating points across {} op types × {} tool types × 5 \
         (Ø, flute) pairs × {} materials",
        points.len(),
        OperationType::ALL.len(),
        ToolType::ALL.len(),
        materials().len()
    );
    println!(
        "  {} of {} ({:.1} %) have chip thinning ACTIVE (combined > 1.0)",
        multiplied.len(),
        points.len(),
        pct(multiplied.len(), points.len())
    );
    println!(
        "  {} of {} ({:.1} %) matched a vendor row that published a band",
        banded.len(),
        points.len(),
        pct(banded.len(), points.len())
    );

    // ── Distribution of the multiplier where it is active ──────────────
    if !multiplied.is_empty() {
        let mut f: Vec<f64> = multiplied.iter().map(|p| p.thinning).collect();
        f.sort_by(|a, b| a.partial_cmp(b).expect("finite factors"));
        let q = |frac: f64| f[((f.len() - 1) as f64 * frac).round() as usize];
        println!(
            "\n  combined multiplier where active:  min {:.3}  p25 {:.3}  median {:.3}  \
             p75 {:.3}  p95 {:.3}  max {:.3}",
            f[0],
            q(0.25),
            q(0.50),
            q(0.75),
            q(0.95),
            f[f.len() - 1]
        );
        let at_ceiling = multiplied
            .iter()
            .filter(|p| p.thinning >= 4.0 - 1e-9)
            .count();
        println!(
            "  {} of {} sit ON the 4.0 clamp ceiling — i.e. the multiplier is not even \
             the geometric value, it is the cap",
            at_ceiling,
            multiplied.len()
        );
    }

    // ── The question that decides safety: where does the band sit? ─────
    let mut over_now = 0usize;
    let mut over_if_reapplied = 0usize;
    let mut under_min_now = 0usize;
    let mut under_min_if_reapplied = 0usize;
    let mut clamped_rows = 0usize;
    for p in &banded {
        if p.clamped {
            clamped_rows += 1;
        }
        if p.over_band(p.advance).is_some_and(|r| r > 1.0 + 1e-9) {
            over_now += 1;
        }
        if p.over_band(p.if_thinning_were_reapplied())
            .is_some_and(|r| r > 1.0 + 1e-9)
        {
            over_if_reapplied += 1;
        }
        if p.under_band(p.advance).is_some_and(|r| r < 1.0 - 1e-9) {
            under_min_now += 1;
        }
        if p.under_band(p.if_thinning_were_reapplied())
            .is_some_and(|r| r < 1.0 - 1e-9)
        {
            under_min_if_reapplied += 1;
        }
    }
    println!(
        "\n  against the DERATED VENDOR BAND, of {} banded points:",
        banded.len()
    );
    println!(
        "    commanded ABOVE band max :  now {} ({:.1} %)   if re-applied {} ({:.1} %)",
        over_now,
        pct(over_now, banded.len()),
        over_if_reapplied,
        pct(over_if_reapplied, banded.len())
    );
    println!(
        "    commanded BELOW band min :  now {} ({:.1} %)   if re-applied {} ({:.1} %)",
        under_min_now,
        pct(under_min_now, banded.len()),
        under_min_if_reapplied,
        pct(under_min_if_reapplied, banded.len())
    );
    // A point clamped UP to the band maximum by the rubbing floor lands
    // exactly on the ceiling, and `apply_feeds_subset` then rounds the feed to
    // 1 mm/min — which can push it a hair over. That is a rounding artefact at
    // a deliberate boundary, not over-feeding, so it is counted separately:
    // conflating the two would overstate the defect by an order of magnitude.
    let over_materially = banded
        .iter()
        .filter(|p| p.over_band(p.advance).is_some_and(|r| r > 1.01))
        .count();
    let over_at_boundary = over_now.saturating_sub(over_materially);
    println!(
        "    of the {over_now} above the maximum, {over_at_boundary} sit within 1 % of it \
         (the rubbing floor clamps to the band ceiling and the feed is then rounded to \
         1 mm/min) and {over_materially} are materially over"
    );
    println!(
        "    {} banded points had a later clamp bind (power / machine ceiling / rubbing \
         floor), so their reconstruction is an UPPER BOUND, not an equality",
        clamped_rows
    );

    // ── The headline: is the feed stack aiming at the vendor band at all? ──
    let tally = |f: &dyn Fn(&Point) -> f64| {
        let (mut inside, mut over, mut under) = (0usize, 0usize, 0usize);
        for p in &banded {
            match p.placement(f(p)) {
                Some("inside") => inside += 1,
                Some("over") => over += 1,
                Some("under") => under += 1,
                _ => {}
            }
        }
        (inside, over, under)
    };
    let now = tally(&|p: &Point| p.advance);
    let before = tally(&|p: &Point| p.if_thinning_were_reapplied());
    println!(
        "\n  WHERE THE COMMANDED FEED LANDS relative to the vendor band \
         ({} banded points):",
        banded.len()
    );
    println!(
        "  {:<38} {:>10} {:>10} {:>10}",
        "", "inside", "over max", "under min"
    );
    for (label, (i, o, u)) in [
        ("as shipped (chip thinning deleted)", now),
        ("if the multiplier were re-applied", before),
    ] {
        println!(
            "  {:<38} {:>4} {:>4.1}% {:>4} {:>4.1}% {:>4} {:>4.1}%",
            label,
            i,
            pct(i, banded.len()),
            o,
            pct(o, banded.len()),
            u,
            pct(u, banded.len())
        );
    }

    // A single scalar for "are we aiming at the band": commanded ÷ band
    // midpoint. 1.0 means dead centre.
    let median_ratio = |f: &dyn Fn(&Point) -> f64| {
        let mut v: Vec<f64> = banded
            .iter()
            .filter_map(|p| {
                p.band.and_then(|(min, max)| {
                    let mid = (min + max) / 2.0;
                    (mid > 0.0).then(|| f(p) / mid)
                })
            })
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        if v.is_empty() {
            f64::NAN
        } else {
            v[v.len() / 2]
        }
    };
    println!(
        "\n  median (commanded ÷ band midpoint):  as shipped {:.3}   if re-applied {:.3}    \
         [1.000 = dead centre of the vendor window]",
        median_ratio(&|p: &Point| p.advance),
        median_ratio(&|p: &Point| p.if_thinning_were_reapplied())
    );

    // ── The worst offenders, which is what a reviewer will want named ──
    let mut worst: Vec<&Point> = banded
        .iter()
        .copied()
        .filter(|p| p.over_band(p.advance).is_some_and(|r| r > 1.0))
        .collect();
    worst.sort_by(|a, b| {
        b.over_band(b.advance)
            .unwrap_or(0.0)
            .partial_cmp(&a.over_band(a.advance).unwrap_or(0.0))
            .expect("finite ratios")
    });
    println!(
        "\n  worst 20 by commanded ÷ band max (the ops Suggest over-feeds against the \
         number the gate judges):"
    );
    println!(
        "  {:<18} {:<16} {:>6} {:>3} {:<12} {:>7} {:>7} {:>7} {:>9} {:>9} {:>6}",
        "op", "tool", "D", "F", "material", "radial", "axial", "comb", "adv", "adv/max", "clamp"
    );
    for p in worst.iter().take(20) {
        println!(
            "  {:<18} {:<16} {:>6.2} {:>3} {:<12} {:>7.3} {:>7.3} {:>7.3} {:>9.5} {:>8.2}× {:>6}",
            format!("{:?}", p.op),
            format!("{:?}", p.tool_kind),
            p.diameter,
            p.flutes,
            p.material,
            p.radial,
            p.axial,
            p.thinning,
            p.advance,
            p.over_band(p.advance).unwrap_or(f64::NAN),
            if p.clamped { "yes" } else { "" }
        );
    }

    // ── Per-op-family rollup, so the blast radius is legible by family ─
    println!("\n  by operation, where thinning is active:");
    println!(
        "  {:<20} {:>7} {:>9} {:>9} {:>13} {:>13}",
        "op", "points", "median", "max", "over max now", "over max after"
    );
    for &op in OperationType::ALL {
        let rows: Vec<&Point> = points
            .iter()
            .filter(|p| p.op == op && p.thinning > 1.0 + 1e-9)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut f: Vec<f64> = rows.iter().map(|p| p.thinning).collect();
        f.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let over_a = rows
            .iter()
            .filter(|p| p.over_band(p.advance).is_some_and(|r| r > 1.0))
            .count();
        let over_b = rows
            .iter()
            .filter(|p| {
                p.over_band(p.if_thinning_were_reapplied())
                    .is_some_and(|r| r > 1.0)
            })
            .count();
        println!(
            "  {:<20} {:>7} {:>9.3} {:>9.3} {:>13} {:>13}",
            format!("{:?}", op),
            rows.len(),
            f[f.len() / 2],
            f[f.len() - 1],
            over_a,
            over_b
        );
    }
    println!();

    // Structural: the numbers printed above must be the ones they are named.
    for p in &points {
        assert!(
            p.thinning >= 1.0 - 1e-9 && p.thinning <= 4.0 + 1e-9,
            "combined chip thinning is documented as clamped to [1, 4]; {:?}/{:?} reports {}",
            p.op,
            p.tool_kind,
            p.thinning
        );
        assert!(
            p.radial >= 1.0 - 1e-9 && p.axial >= 1.0 - 1e-9,
            "both thinning factors are documented as ≥ 1.0; {:?}/{:?} reports radial {} axial {}",
            p.op,
            p.tool_kind,
            p.radial,
            p.axial
        );
    }

    // ── THE SENTRY (G-CHIPTHIN-HALFFIX, ruled 2026-08-19) ──────────────
    //
    // The multiplier must not reach the feed. Asserted as a MECHANISM rather
    // than as a magnitude: the commanded advance per tooth must equal the
    // target chipload composed with the APPLIED derates and nothing else, so
    // re-introducing chip thinning anywhere in the feed expression breaks this
    // regardless of what value it takes.
    //
    // Scoped to points where no later clamp bound, because the power ceiling,
    // the machine cutting-feed clamp and the rubbing floor all truncate the
    // identity by construction — that is the same exclusion
    // `vendor_sidebyside_chipload.rs` makes for the same reason.
    //
    // `spindle_speedup` is in the product and `combined_factor()` deliberately
    // omits it (it walks the constant-chipload line), so it is multiplied back
    // in here.
    //
    // The drill family is excluded, and not as a convenience: calculator
    // Step 9c replaces a drill cycle's feed outright ("a drill cycle has
    // exactly one feed: the plunge") and clamps it into
    // `Material::drill_plunge_feed_envelope_per_mm`. That feed is not the
    // milling expression this identity describes, so including drills would
    // report a 65 % disagreement that is a different code path, not a
    // multiplier. Measured on Ø1.5 in pine: commanded 0.03750 vs 0.02269.
    let mut mechanism_checked = 0usize;
    for p in points.iter().filter(|p| {
        !p.clamped
            && !matches!(
                p.op,
                OperationType::Drill | OperationType::AlignmentPinDrill
            )
    }) {
        let Some(d) = p.derates.as_ref() else {
            continue;
        };
        let predicted = d.target_chip_load_mm * d.combined_factor() * d.spindle_speedup;
        if !(predicted.is_finite() && predicted > 0.0) {
            continue;
        }
        // Tolerance is set by the apply path, not by taste: `apply_feeds_subset`
        // rounds the feed to 1 mm/min before the invariant passes see it, so
        // the commanded advance can sit up to half a mm/min either side of the
        // exact composition. Expressed in advance units that is
        // `0.5 / (rpm × flutes)`. A tighter bar here fails on rounding alone
        // (measured 2.8e-5 on a Ø1.5 end mill in oak) and would say
        // "an unexplained multiplier is in the feed" when there is none.
        let rounding_tolerance = 0.5 / p.divisor;
        let observed = p.calculator_advance;
        let rel = (observed - predicted).abs() / predicted;
        assert!(
            (observed - predicted).abs() <= rounding_tolerance + predicted * 1e-9,
            "{:?} / {:?} Ø{} {}F in {}: the commanded advance does not equal the target \
             chipload times the APPLIED derates.\n  commanded {:.8} mm/tooth\n  predicted \
             {:.8} mm/tooth  (target {:.8} × applied {:.6} × spindle_speedup {:.6})\n  \
             relative disagreement {rel:.6}\n  An unexplained multiplier is in the feed. If \
             it is chip thinning ({:.4}× here), it was deleted on 2026-08-19 by operator \
             ruling and must not return without one — see the Step 5 note in feeds/mod.rs.",
            p.op,
            p.tool_kind,
            p.diameter,
            p.flutes,
            p.material,
            observed,
            predicted,
            d.target_chip_load_mm,
            d.combined_factor(),
            d.spindle_speedup,
            p.thinning
        );
        mechanism_checked += 1;
    }
    assert!(
        mechanism_checked > 100,
        "only {mechanism_checked} points were unclamped enough to check the mechanism — the \
         sweep or the clamp behaviour changed and this sentry is no longer pinning anything"
    );
    println!("  mechanism pinned on {mechanism_checked} unclamped points\n");
}
