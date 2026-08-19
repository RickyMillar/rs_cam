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
//! **That correction landed on the gate and nowhere else.** `feeds/mod.rs`
//! Step 5 still multiplies the feed by `clamp(radial × axial thinning, 1, 4)`,
//! and `feed_modulation.rs` still divides its target by `sqrt(woc)`. So Suggest
//! can command several times the vendor number by design while the post-sim
//! gate judges that same number unmultiplied — and the two are shown side by
//! side on operator surfaces.
//!
//! ## What this file is, and is not
//!
//! It is an **instrument, not a gate.** It measures the magnitude of the
//! multiplication across the shipped operation × tool × material surface and
//! prints the table an operator needs in order to rule on the deletion. The
//! only assertions are structural — that the survey actually swept something,
//! and that the quantities it reports are the ones it claims to report. It
//! deliberately does **not** assert that the multiplication is wrong: that is
//! the operator's call, because it moves every feed in the product.
//!
//! Run it with:
//!
//! ```text
//! cargo test -p rs_cam_core --test chipload_thinning_magnitude_survey -- --nocapture
//! ```
//!
//! ## Reading the counterfactual column
//!
//! Chip thinning enters as a pure multiplier on `raw_feed` at Step 5, so the
//! feed without it is exactly `feed ÷ combined_chip_thinning` — **provided no
//! later clamp bound.** Three can: the power ceiling (Step 6), the machine
//! cutting-feed clamp (Step 7) and the rubbing-floor clamp (Step 9b). Any row
//! where one of those fired is marked `clamped` and its counterfactual is a
//! lower bound rather than an equality. That distinction is the whole reason
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
    /// `combined_chip_thinning` — `clamp(radial × axial, 1, 4)`.
    thinning: f64,
    radial: f64,
    axial: f64,
    /// Commanded advance per tooth as shipped (mm/tooth).
    advance: f64,
    /// Derated vendor band, when the match published one.
    band: Option<(f64, f64)>,
    /// A later clamp bound, so the counterfactual is a lower bound only.
    clamped: bool,
}

impl Point {
    /// The advance the operation would command if Suggest stopped multiplying,
    /// **before** any clamp is re-applied.
    fn counterfactual(&self) -> f64 {
        self.advance / self.thinning
    }

    /// The same, with Step 9b re-applied — the honest counterfactual, because
    /// removing a multiplier lowers the commanded advance and so makes the
    /// rubbing-floor clamp fire far more often than it does today. Without
    /// this the deletion looks like it drives everything into rubbing, when in
    /// fact the existing floor catches part of the fall.
    fn counterfactual_floored(&self) -> f64 {
        self.counterfactual()
            .max(rs_cam_core::feeds::effective_rubbing_floor(self.band.map(
                |(min_mm_per_tooth, max_mm_per_tooth)| rs_cam_core::feeds::ChiploadBounds {
                    min_mm_per_tooth,
                    max_mm_per_tooth,
                },
            )))
    }

    /// Thinning deleted **and** the seed target moved from the band midpoint
    /// (`SuggestAggressiveness::Default`) to the band maximum
    /// (`SuggestAggressiveness::Speed`), floor re-applied. The target enters
    /// the feed expression linearly, so this is an exact rescale of the
    /// counterfactual by `max ÷ midpoint` — no re-run needed. Included because
    /// "delete the multiplier" and "aim at the right part of the band" are
    /// separate levers, and the survey should not force them into one.
    fn counterfactual_at_band_max(&self) -> f64 {
        let scaled = self.band.map_or(self.counterfactual(), |(min, max)| {
            let mid = (min + max) / 2.0;
            if mid > 0.0 {
                self.counterfactual() * max / mid
            } else {
                self.counterfactual()
            }
        });
        scaled.max(rs_cam_core::feeds::effective_rubbing_floor(self.band.map(
            |(min_mm_per_tooth, max_mm_per_tooth)| rs_cam_core::feeds::ChiploadBounds {
                min_mm_per_tooth,
                max_mm_per_tooth,
            },
        )))
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
                        thinning: r.derates.combined_chip_thinning,
                        radial: r.derates.radial_chip_thinning,
                        axial: r.derates.axial_chip_thinning,
                        advance,
                        band: r
                            .chipload_bounds
                            .map(|b| (b.min_mm_per_tooth, b.max_mm_per_tooth)),
                        clamped,
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
fn chip_thinning_multiplication_magnitude_and_counterfactual() {
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
    let mut over_counterfactual = 0usize;
    let mut under_min_now = 0usize;
    let mut under_min_counterfactual = 0usize;
    let mut clamped_rows = 0usize;
    for p in &banded {
        if p.clamped {
            clamped_rows += 1;
        }
        if p.over_band(p.advance).is_some_and(|r| r > 1.0 + 1e-9) {
            over_now += 1;
        }
        if p.over_band(p.counterfactual())
            .is_some_and(|r| r > 1.0 + 1e-9)
        {
            over_counterfactual += 1;
        }
        if p.under_band(p.advance).is_some_and(|r| r < 1.0 - 1e-9) {
            under_min_now += 1;
        }
        if p.under_band(p.counterfactual())
            .is_some_and(|r| r < 1.0 - 1e-9)
        {
            under_min_counterfactual += 1;
        }
    }
    println!(
        "\n  against the DERATED VENDOR BAND, of {} banded points:",
        banded.len()
    );
    println!(
        "    commanded ABOVE band max :  now {} ({:.1} %)   without thinning {} ({:.1} %)",
        over_now,
        pct(over_now, banded.len()),
        over_counterfactual,
        pct(over_counterfactual, banded.len())
    );
    println!(
        "    commanded BELOW band min :  now {} ({:.1} %)   without thinning {} ({:.1} %)",
        under_min_now,
        pct(under_min_now, banded.len()),
        under_min_counterfactual,
        pct(under_min_counterfactual, banded.len())
    );
    println!(
        "    {} banded points had a later clamp bind (power / machine ceiling / rubbing \
         floor), so their counterfactual is a LOWER BOUND, not an equality",
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
    let bare = tally(&|p: &Point| p.counterfactual());
    let floored = tally(&|p: &Point| p.counterfactual_floored());
    let at_max = tally(&|p: &Point| p.counterfactual_at_band_max());
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
        ("as shipped today", now),
        ("thinning deleted, no floor re-applied", bare),
        ("thinning deleted, Step-9b floor applied", floored),
        ("thinning deleted, seed at band max + floor", at_max),
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
        "\n  median (commanded ÷ band midpoint):  today {:.3}   deleted {:.3}   \
         deleted+floor {:.3}    [1.000 = dead centre of the vendor window]",
        median_ratio(&|p: &Point| p.advance),
        median_ratio(&|p: &Point| p.counterfactual()),
        median_ratio(&|p: &Point| p.counterfactual_floored())
    );
    println!(
        "  median with the seed at band max instead of midpoint: {:.3}",
        median_ratio(&|p: &Point| p.counterfactual_at_band_max())
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
            .filter(|p| p.over_band(p.counterfactual()).is_some_and(|r| r > 1.0))
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

    // Structural assertions only — this is an instrument, not a gate. What it
    // must guarantee is that the numbers above are the ones it names.
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
}
