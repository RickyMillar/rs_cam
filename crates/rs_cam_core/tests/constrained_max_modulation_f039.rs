//! F-039 — Constrained-max feed modulation acceptance bars.
//!
//! Six pure-algorithm acceptance tests + the transparency-surface
//! integration check. Drives the modulator directly via
//! `adaptive_feed_modulate` with synthetic toolpaths so the bars
//! don't depend on simulator output.
//!
//! Tests:
//!  1. `constrained_max_emits_max_feed_when_chipload_max_binds` —
//!     full slot engagement, generous machine + power + deflection
//!     headroom → binding constraint is `ChiploadMax`, emitted
//!     feed = `band.max × RPM × flutes`.
//!  2. `constrained_max_raises_feed_when_under_band_on_light_engagement`
//!     — light engagement, commanded feed below band → modulator
//!     scales up toward the chipload-max limit.
//!  3. `constrained_max_binds_on_deflection_for_long_tool` —
//!     deflection inputs configured for a thin long tool → binding
//!     constraint is `DeflectionMax`.
//!  4. `constrained_max_binds_on_power_for_low_rpm` — power inputs
//!     configured for a low-power band → binding constraint is
//!     `PowerMax`.
//!  5. `aggressiveness_below_one_emits_proportional_feed` —
//!     aggressiveness 1.0 vs 0.7 emits ~70 % of the at-limit feed
//!     (provided the chipload-min floor doesn't trip).
//!  6. `modulation_summary_matches_per_move_binding_distribution` —
//!     `ModulationOutcome::build_summary` returns a distribution
//!     whose entries match a hand-computed histogram.
//!
//! See `planning/acceptance_loop/findings/F-039-constrained-max-feed-modulation.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feed_modulation::{
    BindingConstraint, ChiploadBand, DeflectionLimitInputs, ModulationContext, ModulationStrategy,
    PerMoveEngagement, PowerLimitInputs, adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

fn shapeoko() -> MachineKinematics {
    MachineKinematics::shapeoko_xxl_stock()
}

fn band(min: f64, max: f64) -> ChiploadBand {
    ChiploadBand::new(min, max).expect("valid band")
}

fn ctx_basic<'a>(k: &'a MachineKinematics, b: ChiploadBand) -> ModulationContext<'a> {
    ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        max_feed_mm_min: 10_000.0,
        rapid_feed_mm_min: 10_000.0,
        chipload_band: b,
        kinematics: k,
        strategy: ModulationStrategy::ConstrainedMax,
        aggressiveness: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        nominal_axial_doc_mm: 2.0,
    }
}

fn straight_toolpath(n_cuts: usize, feed_mm_min: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 0..n_cuts {
        let x = (i + 1) as f64 * 50.0;
        tp.feed_to_with_intent(P3::new(x, 0.0, -2.0), feed_mm_min, MoveIntent::ClearingCut);
    }
    tp
}

// ── 1. chipload-max binds at full slot ─────────────────────────────

#[test]
fn constrained_max_emits_max_feed_when_chipload_max_binds() {
    let mut tp = straight_toolpath(3, 1500.0);
    let engagements: Vec<_> = (0..tp.moves.len())
        .map(|i| {
            if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                }
            }
        })
        .collect();
    let k = shapeoko();
    let ctx = ctx_basic(&k, band(0.02, 0.08));
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();

    // Pick any cutting move's binding constraint.
    let mut saw_chipload_max = false;
    for (feed, binding) in outcome.per_move.values() {
        if *binding == BindingConstraint::ChiploadMax {
            // At full slot, no chip-thinning: feed = 0.08 × 18000 × 2 = 2880.
            assert!(
                (feed - 2880.0).abs() < 1.0,
                "expected ~2880 at chipload-max bind, got {feed}"
            );
            saw_chipload_max = true;
        }
    }
    assert!(
        saw_chipload_max,
        "expected at least one move bound by chipload-max"
    );
}

// ── 2. raises feed on light engagement ─────────────────────────────

#[test]
fn constrained_max_raises_feed_when_under_band_on_light_engagement() {
    let mut tp = straight_toolpath(3, 800.0); // very low commanded
    let engagements: Vec<_> = (0..tp.moves.len())
        .map(|i| {
            if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 0.25,
                    axial_doc_fraction: 1.0,
                }
            }
        })
        .collect();
    let k = shapeoko();
    let ctx = ctx_basic(&k, band(0.02, 0.08));
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();

    // With chip-thinning at WOC=0.25, target = 0.08 / sqrt(0.25) = 0.16.
    // feed = 0.16 × 18000 × 2 = 5760 mm/min — way above commanded 800.
    for (idx, m) in tp.moves.iter().enumerate() {
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        let f = m.move_type.feed_rate().unwrap();
        assert!(
            f > 800.0,
            "move {idx} feed {f} should be raised above commanded 800"
        );
    }
    assert!(outcome.changed >= 1);
}

// ── 3. deflection binds for long tool ──────────────────────────────

#[test]
fn constrained_max_binds_on_deflection_for_long_tool() {
    let mut tp = straight_toolpath(2, 1500.0);
    let engagements: Vec<_> = (0..tp.moves.len())
        .map(|i| {
            if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                }
            }
        })
        .collect();
    let k = shapeoko();
    let mut ctx = ctx_basic(&k, band(0.02, 0.08));
    // Stiffness-limited small tool: 3 mm diameter, 60 mm stickout
    // (L/D = 20). Feed-aware affine coefficients scaled to Kc ≈ 30
    // (Ks = 49.95·30/35.1 ≈ 42.7, F_edge = 5.30·30/35.1 ≈ 4.53) and the
    // beam compliance for a uniform 3 mm cantilever, load at ~59 mm:
    // a²·(3L−a)/(6·E·I) ≈ 0.029 mm/N. At full chipload-max + 2 mm axial
    // DOC + full slot the tip deflects ~0.47 mm > 0.2 mm, so DeflectionMax
    // binds.
    ctx.deflection_inputs = Some(DeflectionLimitInputs {
        ks_n_per_mm2: 42.7,
        f_edge_n_per_mm: 4.53,
        compliance_mm_per_n: 0.029,
        max_tip_deflection_mm: 0.2,
    });
    ctx.nominal_axial_doc_mm = 2.0;
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();

    let mut saw_deflection = false;
    for (_feed, binding) in outcome.per_move.values() {
        if *binding == BindingConstraint::DeflectionMax {
            saw_deflection = true;
        }
    }
    assert!(
        saw_deflection,
        "expected deflection-max to bind on long thin tool. Per-move map: {:?}",
        outcome.per_move
    );
}

// ── 4. power binds for low-power machine ───────────────────────────

#[test]
fn constrained_max_binds_on_power_for_low_rpm() {
    let mut tp = straight_toolpath(2, 1500.0);
    let engagements: Vec<_> = (0..tp.moves.len())
        .map(|i| {
            if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                }
            }
        })
        .collect();
    let k = shapeoko();
    let mut ctx = ctx_basic(&k, band(0.02, 0.08));
    // Very low available power (10 W) with a 6 mm cutter in
    // hardwood → power binds hard. Pre-S2-9 (2026-05-31) this fixture
    // passed a pre-multiplied `kc_eff_n_per_mm2: 60.0` (= 2.0 × 30).
    // Post-S2-9 the field is raw Kc and the solver applies
    // GRAIN_ANISOTROPY_FACTOR internally — same effective product
    // (2.0 × 30 = 60), one less literal to keep in sync with future
    // anisotropy-factor changes.
    ctx.power_inputs = Some(PowerLimitInputs {
        kc_n_per_mm2: 30.0,
        engagement_diameter_mm: 6.0,
        available_kw: 0.01,
    });
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();

    let mut saw_power = false;
    for (_feed, binding) in outcome.per_move.values() {
        if *binding == BindingConstraint::PowerMax {
            saw_power = true;
        }
    }
    assert!(
        saw_power,
        "expected power-max to bind on low-power fixture. Per-move map: {:?}",
        outcome.per_move
    );
}

// ── 5. aggressiveness scales proportionally ────────────────────────

#[test]
fn aggressiveness_below_one_emits_proportional_feed() {
    // High band ceiling + low chipload-min so the floor doesn't trip
    // at 0.7 aggressiveness. Use band (0.005, 0.08) → floor =
    // 0.005×18000×2 = 180 mm/min, well below 0.7 × ~2880 ≈ 2016.
    let make_tp = || straight_toolpath(2, 1500.0);

    let k = shapeoko();
    let mut ctx_a = ctx_basic(&k, band(0.005, 0.08));
    ctx_a.aggressiveness = 1.0;
    let mut ctx_b = ctx_a;
    ctx_b.aggressiveness = 0.7;

    let mut tp_a = make_tp();
    let mut tp_b = make_tp();
    let engagements: Vec<_> = (0..tp_a.moves.len())
        .map(|i| {
            if matches!(tp_a.moves[i].move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                }
            }
        })
        .collect();

    adaptive_feed_modulate(&mut tp_a, &engagements, &ctx_a).unwrap();
    adaptive_feed_modulate(&mut tp_b, &engagements, &ctx_b).unwrap();

    // Compare the first cutting move's feed across both.
    let f_a = tp_a.moves[1].move_type.feed_rate().unwrap();
    let f_b = tp_b.moves[1].move_type.feed_rate().unwrap();
    let ratio = f_b / f_a;
    assert!(
        (ratio - 0.7).abs() < 0.02,
        "expected ~0.7 ratio, got {ratio} (f_a={f_a}, f_b={f_b})"
    );
}

// ── 6. summary distribution matches per-move map ───────────────────

#[test]
fn modulation_summary_matches_per_move_binding_distribution() {
    // Use a heterogeneous engagement profile so multiple bindings
    // appear in the per-move map.
    let mut tp = straight_toolpath(4, 1500.0);
    let engagements: Vec<_> = (0..tp.moves.len())
        .map(|i| {
            if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else if i % 2 == 0 {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                }
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 0.3,
                    axial_doc_fraction: 1.0,
                }
            }
        })
        .collect();
    let k = shapeoko();
    let mut ctx = ctx_basic(&k, band(0.02, 0.08));
    ctx.max_feed_mm_min = 3000.0; // force machine cap to bind sometimes
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
    let commanded_feed = 1500.0_f64;
    let summary = outcome
        .build_summary(commanded_feed, ctx.aggressiveness, ctx.strategy)
        .expect("summary present when moves were visited");

    // Hand-count the per-move map.
    let mut hand: std::collections::BTreeMap<BindingConstraint, usize> =
        std::collections::BTreeMap::new();
    for (_, b) in outcome.per_move.values() {
        *hand.entry(*b).or_insert(0) += 1;
    }
    let total = outcome.per_move.len();
    assert!(total > 0, "expected at least one visited move");
    assert_eq!(summary.moves_total, total);

    for (b, count) in &hand {
        let fraction = (*count as f64) / (total as f64);
        let from_summary = summary
            .binding_constraint_distribution
            .get(b)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (fraction - from_summary).abs() < 1e-6,
            "binding {b:?}: hand {fraction} vs summary {from_summary}"
        );
    }
    assert_eq!(summary.aggressiveness, 1.0);
}
