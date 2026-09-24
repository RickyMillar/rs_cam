//! **A-5 (F-T35) — the `arc_fit_ratio_for_op` evidence package.**
//!
//! # What is being measured and why
//!
//! `feeds::predict::arc_fit_ratio_for_op` carries a per-operation-family
//! table (Adaptive3d 0.25, DropCutter 0.15, `Calibrated`; everything else a
//! `Default`) that was fitted against the post-sim chipload gate's **arc-mean
//! chip thickness** observation. That observation was **deleted on
//! 2026-08-06**: the gate now reports `effective_feed / (rpm · flutes)`, a
//! linear advance per tooth (`feeds::feed_explanation`'s module header, and
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` for the
//! primary sources).
//!
//! The table did not follow. `suggest::recalibrate_feed_for_chipload` still
//! solves
//!
//! ```text
//! feed = target / arc_fit_ratio × rpm × flutes
//! ```
//!
//! and is gated on `ArcFitRatioSource::Calibrated`, so exactly two families
//! — Adaptive3d and DropCutter — get an automatic feed lift, sized by
//! `1 / ratio`, aimed at a quantity nothing reports any more.
//!
//! This harness measures what that costs, on the real Suggest path, and what
//! each of the three dispositions the operator review put on the table would
//! do to the shipped recommendation. It is the evidence for **Checkpoint J**.
//!
//! # The three dispositions, as arms
//!
//! | arm | disposition | feed applied to the measured op |
//! |---|---|---|
//! | **A — shipped** | today's behaviour | Suggest's own recommended feed, lift included |
//! | **B — retire** | (a) retire the automatic feed-up entirely | `FeedRaisedForChipload::requested_mm_per_min`, i.e. the feed Suggest had *before* pass 8 |
//! | **C — re-key to 1.0** | the reference point the 4×/6.7× claim is stated against | `min(target × rpm × flutes, cutting ceiling)` — the lift the *current* gate observation would justify |
//!
//! Disposition (b) — demote to a clearly-marked estimated mode — moves no
//! number and therefore has no arm: it is arm A with a label, which is why
//! the report-tier wording change ships separately and before the ruling.
//! Disposition (c) — move the action into the simulation-backed optimizer —
//! has no arm either, for a stronger reason: Suggest would emit arm B and
//! the optimizer would retarget from the *measured* gate observation, which
//! is arm B's measured column in this file. Both are read off the arms that
//! exist rather than being simulated as fourth and fifth arms.
//!
//! # Instrument integrity
//!
//! * Suggest-side numbers come from `ProjectSession::cutter_op_profile` — the
//!   same assembly the GUI Suggest button, the MCP rationale endpoint and the
//!   CLI `--apply-suggest` path use — never from a hand-computed formula.
//! * Sim-side numbers come from `ToolpathLoadVerdict::feed_explanation`,
//!   the stage-labelled record: stage 1 commanded advance/tooth, stage 2
//!   vendor band, stage 3 achieved/commanded feed ratio, stage 4 the gate's
//!   own observation. All four in `ADVANCE_PER_TOOTH`, so every ratio in the
//!   report is a like-for-like comparison.
//! * Every arm reads `ToolpathStats::stock_snapshot` (S-4's `StockSnapshotStamp`)
//!   and the A/B/C comparison **asserts the three arms consumed the same
//!   machined stock**. The arms differ in feed only; feed does not move
//!   geometry, so an unequal stamp would mean the comparison was unfair
//!   before any conclusion about feed could be drawn.
//! * The simulation cell is a named constant and is reported beside every
//!   measurement.
//!
//! # 2026-08-13 — the disposition SHIPPED (Checkpoint J-1/J-5)
//!
//! The operator ruled disposition **(a) now, (c) as the destination, as one
//! change**: the automatic feed-up is retired, Suggest emits the un-lifted
//! feed, and any lift belongs to the simulation-backed path. This file was
//! written before that ruling and is kept as the permanent record of what
//! was wrong, so it had to be re-pointed rather than re-baselined:
//!
//! * **Arm A is now a PINNED CONSTANT per fixture** (`legacy_shipped_feed`:
//!   6912.0 / 6000.0 / 4405.0 / 2500.0 mm/min), not a value read back out of
//!   Suggest. That is what lets the pre-fix reproduction survive the fix —
//!   `arc_fit_arms_gate_observation` still asserts `arm A == Exceeds` on all
//!   four fixtures, because arm A is a feed this harness applies explicitly.
//! * **Arm B is now what Suggest returns.** Pre-fix it was
//!   `FeedRaisedForChipload::requested_mm_per_min`, i.e. Suggest's feed with
//!   pass 8 subtracted. Post-fix Suggest simply emits it. The new sentry
//!   `retired_lift_leaves_feed_at_the_calculator_value` asserts the two are
//!   the same number — which is the whole claim of disposition (a).
//! * **Arm C's target is now computed from the band** rather than read off
//!   the (no longer emitted) warning: `SuggestAggressiveness::Default` aims
//!   at the band midpoint, and the pre-fix run's four `lut_target` values
//!   reproduce as midpoints to 5 dp.
//!
//! # Fixture shape
//!
//! Every fixture is a two-op cascade on the same synthetic surface: op 0 is a
//! fresh-stock rough, op 1 is the **measured** op reading the stock op 0 left
//! (`StockSource::FromRemainingStock`). The cascade is what makes the
//! snapshot stamp `Some` — a fresh-stock generation consumes no machined
//! stock and would stamp `None`, making the equality assertion vacuous.
//!
//! **2026-09-25 — the rough leaves a skin.** Op 0 has one level, at the
//! bottom height plus its stock-to-leave. Until `93ce4e29` the Global level
//! gate skipped that level: no surface cell was at or below it. Op 0 was then
//! nearly empty (155 mm of cut), and op 1 cut near-fresh stock. After the
//! gate fix, op 0 drapes onto the surface (1608 mm of cut). A3D-2 (Ø8) then
//! found no material above its own floor and emitted 0 moves, so its gate
//! had no population. Op 0 now leaves [`ROUGH_STOCK_TO_LEAVE`], and op 1
//! reads a skin of real material.
//!
//! Two fixtures per affected family, differing in tool, species and machine
//! feed ceiling, because whether the machine ceiling binds decides whether
//! the lift is realised at all.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::{ball_tool_config, endmill_tool_config, tapered_ball_tool_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, DropCutterConfig,
};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::compute::toolpath_stats::StockSnapshotStamp;
use rs_cam_core::feeds::suggest::SuggestWarning;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, ReplaceToolpathConfigArgs, SimulationOptions,
};

// ── Fixture constants ───────────────────────────────────────────────────

/// Half-extent (mm) of the fixture surface.
const HALF: f64 = 15.0;

/// Stock height (mm).
const STOCK_Z: f64 = 8.0;

/// Peak-to-trough of the fixture surface (mm).
const RELIEF: f64 = 3.0;

/// Axial stock-to-leave of op 0, the upstream rough (mm).
///
/// It is larger than the measured op's 0.5 mm default, so op 0 leaves a skin
/// for op 1 to cut. With the default on both ops, the Ø8 measured op finds no
/// material above its floor: the Ø6 rough already cut to that floor or below.
const ROUGH_STOCK_TO_LEAVE: f64 = 1.5;

/// The simulation cell (mm). Reported beside every measurement, per §0
/// rule 10 — a collision count or an engagement reading without its cell
/// is not a measurement.
const SIM_CELL_MM: f64 = 0.4;

/// A smooth double bump occupying the top `RELIEF` mm of the stock, so the
/// rough leaves real material for the measured op to cut.
fn bumpy_surface() -> rs_cam_core::mesh::TriangleMesh {
    height_field(HALF, 0.5, |x, y| {
        let sx = (x / HALF * std::f64::consts::PI).cos();
        let sy = (y / HALF * std::f64::consts::PI).cos();
        STOCK_Z - RELIEF * 0.5 * (1.0 - sx * sy * 0.9)
    })
}

// ── Fixtures ────────────────────────────────────────────────────────────

struct Fixture {
    label: &'static str,
    family: &'static str,
    material: Material,
    /// Tool for op 0 (the upstream rough). Always a flat endmill.
    rough_tool: ToolConfig,
    /// Tool for op 1 (the measured op).
    tool: ToolConfig,
    /// The measured operation, with `spindle_rpm` pinned.
    op: OperationConfig,
    /// RPM pinned on the measured op. Every arm applies this same RPM, so
    /// the A/B/C comparison is attributable to the feed alone.
    rpm: u32,
    /// `max_feed_mm_min` for the fixture's machine.
    max_feed_mm_min: f64,
    /// Explicit `max_cutting_feed_mm_min`. `None` leaves the conservative
    /// derived ceiling `min(max_feed, DEFAULT_CUTTING_FEED_CAP_MM_MIN = 6000)`.
    ///
    /// This dial is load-bearing for the evidence, not fixture decoration:
    /// whether the ceiling binds decides whether the arc-fit lift is
    /// *realised* or merely *solved for*, and the two differ by a lot.
    max_cutting_feed_mm_min: Option<f64>,
    /// **Arm A — the feed Suggest SHIPPED before the retirement** (mm/min),
    /// measured on this exact fixture at `719b92d7` and pinned here on
    /// 2026-08-13. Applied explicitly by `arc_fit_arms_gate_observation`
    /// so the pre-fix reproduction outlives the code that produced it.
    legacy_shipped_feed: f64,
    /// **Arm B — the calculator's own feed** (mm/min): pre-fix this was
    /// `FeedRaisedForChipload::requested_mm_per_min`; post-fix it is what
    /// `suggested_operation.feed_rate()` returns. The retirement sentry
    /// asserts Suggest now lands here.
    expected_suggest_feed: f64,
}

fn rough_tool() -> ToolConfig {
    let mut t = endmill_tool_config(6.0);
    t.stickout = 20.0;
    t.flute_count = 2;
    t
}

fn adaptive3d_op(feed: f64, rpm: u32, stepover: f64, dpp: f64) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover,
        depth_per_pass: dpp,
        feed_rate: feed,
        plunge_rate: 300.0,
        spindle_rpm: Some(rpm),
        // Helix entry so the fixture does not also trip
        // `PlungeEntryUnstableAtDpp` and confound the warning list.
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    })
}

fn drop_cutter_op(feed: f64, rpm: u32, stepover: f64) -> OperationConfig {
    OperationConfig::DropCutter(DropCutterConfig {
        stepover,
        feed_rate: feed,
        plunge_rate: 300.0,
        spindle_rpm: Some(rpm),
        min_z: STOCK_Z - RELIEF - 1.0,
        ..DropCutterConfig::default()
    })
}

fn fixtures() -> Vec<Fixture> {
    let mut a3d_2_tool = endmill_tool_config(8.0);
    a3d_2_tool.stickout = 25.0;
    a3d_2_tool.flute_count = 3;

    let mut a3d_1_tool = endmill_tool_config(6.0);
    a3d_1_tool.stickout = 18.0;
    a3d_1_tool.flute_count = 2;

    let mut dc_1_tool = ball_tool_config(3.0);
    dc_1_tool.stickout = 20.0;
    dc_1_tool.flute_count = 2;

    let mut dc_2_tool = tapered_ball_tool_config(2.0, 7.0, 6.0);
    dc_2_tool.stickout = 30.0;
    dc_2_tool.flute_count = 2;

    vec![
        // Uncapped: isolates the closed form, so the 1/ratio claim is
        // measured rather than inferred through a ceiling.
        Fixture {
            label: "A3D-1 Ø6 2F endmill / HardMaple / raised cutting ceiling",
            family: "Adaptive3d",
            material: Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            rough_tool: rough_tool(),
            tool: a3d_1_tool,
            op: adaptive3d_op(900.0, 16_000, 1.2, 2.0),
            rpm: 16_000,
            max_feed_mm_min: 15_000.0,
            max_cutting_feed_mm_min: Some(15_000.0),
            legacy_shipped_feed: 6912.0,
            // **RE-PINNED 2026-08-13, A-7 / Checkpoint K-(a4): 1215.0 →
            // 918.0** (×0.7556). Mechanism: a4 hoisted the LUT routing
            // into one `vendor_normalize::lut_query_for` that BOTH
            // consumers call, so Suggest now queries `Adaptive3d` under
            // the `Pocket` family — the family the gate has always used.
            // The matched band moves 0.038–0.070
            // (`amana-flat-hardwood-adaptive-6000-2f`) → 0.032–0.055
            // (`…-pocket-6000-2f`), and the recipe follows it. This is
            // the reference case A-6 measured at ×1.2727 on the band
            // maximum; 1/1.2727 = 0.786 against an observed 0.7556, the
            // remainder being the DOC derate applied to the new band.
            //
            // **RE-PINNED AGAIN 2026-08-19, G-CHIPTHIN-HALFFIX: 1223.4 →
            // 1000.0** (÷1.2234, this fixture's `combined_chip_thinning`).
            // Suggest no longer multiplies the feed by chip thinning at all —
            // operator-ruled after the magnitude survey, on the finding that
            // no wood chart in the LUT publishes a radial condition for its
            // chipload column. Commanded advance 0.04078 → **0.03333
            // mm/tooth** against a derated band of 0.032–0.055: still inside
            // the vendor window, now near its lower edge rather than its
            // middle. That is the intended direction — the deletion trades
            // "middle of the band on an unsourced multiplier" for "low in the
            // band on the vendor's own number".
            //
            // Superseded note, kept for lineage:
            // **RE-PINNED 2026-08-19, G-SUGGEST-NOCLAMP: 918.0 → 1223.4**
            // (×4/3, exactly). Suggest pass 9 now re-derives the feed at the
            // geometry the operation ships, and the axial envelope clamps
            // this op's DPP across the 1×D depth-tier boundary on a Ø6 tool
            // — tier 0.75 → 1.00 — so the derate the calculator had folded
            // in no longer applies. Justified by the band, not by the test:
            // the commanded advance moves 0.03060 → 0.04078 mm/tooth against
            // a derated band of 0.032–0.055 with target 0.04350. It was
            // BELOW the vendor minimum and now sits inside the band near its
            // target. Nothing about the retired arc-fit lift changed —
            // point 3 still holds at 5.65×, and arm C is reconstructed from
            // the band and RPM, so point 4 is untouched.
            //
            // **RE-PINNED 2026-09-24, feeds matrix R5 + ruling R4 WP2a:
            // 1000.0 → 3429.0**, the measured un-lifted calculator feed on the
            // 2026-09-24 rows. The Ø6 hardwood query now resolves the
            // one-value Spektra row (max 0.127, no minimum), so Suggest carries
            // no band. At 18 000 rpm x 2 flutes the commanded advance is
            // 3429 / 36 000 = 0.09525 mm/tooth, under the row's 0.127 and
            // still strictly below the legacy 6912.0 (point 3).
            //
            // **RE-PINNED 2026-09-24, ruling R4 WP3: 3429.0 → 4572.0.** The
            // 0.75 machine factor left the feed: 3429 / 0.75 = 4572 =
            // 0.127 x 18 000 x 2, the printed row value itself (advance
            // 0.127 mm/tooth, no factor).
            expected_suggest_feed: 4572.0,
        },
        // Stock ceiling: what an operator on a normal machine actually gets.
        Fixture {
            label: "A3D-2 Ø8 3F endmill / WhiteOak / stock ceiling",
            family: "Adaptive3d",
            material: Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            rough_tool: rough_tool(),
            tool: a3d_2_tool,
            op: adaptive3d_op(1100.0, 14_000, 1.6, 2.0),
            rpm: 14_000,
            max_feed_mm_min: 8_000.0,
            max_cutting_feed_mm_min: None,
            legacy_shipped_feed: 6000.0,
            // **RE-PINNED 2026-08-13, A-7 / Checkpoint K-(a4): 2243.0 →
            // 1694.0** (×0.7552) — the same mechanism and, to three
            // figures, the same factor as A3D-1, which is what one
            // expects from one row-pair carried by the same scale terms.
            //
            // **RE-PINNED AGAIN 2026-08-19, G-CHIPTHIN-HALFFIX: 2258.4 →
            // 1806.7** (÷1.25). Commanded advance 0.05019 → **0.04015
            // mm/tooth** against a derated band of 0.03938–0.06768 — inside,
            // near the lower edge, same story as A3D-1.
            //
            // Superseded note, kept for lineage:
            // **RE-PINNED 2026-08-19, G-SUGGEST-NOCLAMP: 1694.0 → 2258.4**
            // (×4/3, exactly — the same depth-tier crossing as A3D-1, on Ø8
            // rather than Ø6). Commanded advance 0.03764 → 0.05019 mm/tooth
            // against a derated band of 0.03938–0.06768, target 0.05353:
            // again below the vendor minimum before, inside the band and
            // close to target after.
            //
            // The two DropCutter fixtures below are deliberately NOT
            // re-pinned. Surface-following ops command no axial step, so
            // their depth-tier term is identical at both operating points
            // and cancels; pass 9 finds no mutated geometry and does not
            // fire. That asymmetry — the DPP-clamped family moves, the
            // family with no DPP does not — is itself the check that the
            // pass is keyed on geometry and not on op family.
            // **RE-PINNED 2026-09-24, feeds matrix R5 + ruling R4 WP2a:
            // 1806.7 → 4500.0**, the measured un-lifted calculator feed on the
            // 2026-09-24 rows. Suggest carries no band here either (a
            // one-value row), and the feed is below the 6000 mm/min stock
            // ceiling: 4500 / (17 080 x 3) = 0.0878 mm/tooth.
            //
            // **RE-PINNED 2026-09-24, ruling R4 WP3: 4500.0 → 6000.0.** With
            // the 0.75 gone the un-capped feed is 4500 / 0.75 = 6000, which
            // is exactly the 6000 mm/min stock cutting ceiling. It equals the
            // legacy shipped feed, which the same ceiling also bound.
            expected_suggest_feed: 6000.0,
        },
        Fixture {
            label: "DC-1 Ø3 ball / HardMaple / stock ceiling",
            family: "DropCutter",
            material: Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            rough_tool: rough_tool(),
            tool: dc_1_tool,
            op: drop_cutter_op(1200.0, 18_000, 0.35),
            rpm: 18_000,
            max_feed_mm_min: 12_000.0,
            max_cutting_feed_mm_min: None,
            legacy_shipped_feed: 4405.0,
            // **RE-PINNED 2026-08-19, G-CHIPTHIN-HALFFIX: 1487.0 → 881.0**
            // (÷1.688). Unlike the two Adaptive3d fixtures this one does NOT
            // land mid-band: its derated band is 0.01159–0.02318, entirely
            // below the 0.025 mm/tooth chip-formation floor, so
            // `effective_rubbing_floor` collapses to the band CEILING and
            // Step 9b pins the commanded advance there — **0.02318 mm/tooth,
            // exactly `max`**. A hardwood ball tool cannot both clear chip
            // formation and stay inside its vendor window, which is the
            // FEEDS_CENSUS C-12 tradeoff, disclosed rather than hidden.
            //
            // **RE-PINNED 2026-09-18, T-9: 881.0 → 880.0** (−1 mm/min).
            // `apply_feeds_subset` now calls `round_suggestion_value_down`,
            // which FLOORS the feed instead of rounding it to the nearest, so
            // a clamped feed can no longer ship above its ceiling. Pass 9
            // does not fire on a DropCutter — no axial step, no mutated
            // geometry — so this fixture ships the quantised value directly
            // and moves by the full step. Measured, not adjusted.
            //
            // **RE-PINNED 2026-09-24, ruling R4 WP2a: 880.0 → 371.0**, the
            // measured un-lifted calculator feed on the 2026-09-24 rows.
            // Step 9b no longer lifts the advance to the band ceiling. At
            // 19 000 rpm x 2 flutes the commanded advance is 371 / 38 000 =
            // 0.00976 mm/tooth, under the band 0.011592-0.023184 and under
            // the floor, so the recipe carries the rubbing-floor warning.
            // Modulation lifts it into the band on the sim side (gate 0.02318).
            //
            // **RE-PINNED 2026-09-24, ruling R4 WP3: 371.0 → 660.0.** The 0.75
            // machine factor and the 0.75 long-tool share left the feed:
            // 371 / (0.75 x 0.75) = 659.6, measured 660. The advance is
            // 660 / 38 000 = 0.01737 mm/tooth, inside the band
            // 0.011592-0.023184, so the Q9 floor (the band minimum) does not
            // warn.
            expected_suggest_feed: 660.0,
        },
        // A small router: the ceiling binds on a DropCutter lift too.
        Fixture {
            label: "DC-2 Ø2-tip tapered ball / WhiteOak / small router",
            family: "DropCutter",
            material: Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            rough_tool: rough_tool(),
            tool: dc_2_tool,
            op: drop_cutter_op(1000.0, 20_000, 0.25),
            rpm: 20_000,
            max_feed_mm_min: 2_500.0,
            max_cutting_feed_mm_min: None,
            legacy_shipped_feed: 2500.0,
            // **RE-PINNED 2026-08-19, G-CHIPTHIN-HALFFIX: 1029.0 → 638.0**
            // (÷1.613). Same rubbing-floor-at-the-band-ceiling outcome as
            // DC-1: band 0.00839–0.01679, commanded lands on **0.01679
            // mm/tooth, exactly `max`**.
            //
            // **RE-PINNED 2026-09-18, T-9: 638.0 → 637.0** (−1 mm/min), the
            // same cause as DC-1 above: the apply path floors the feed.
            //
            // **RE-PINNED 2026-09-24, feeds matrix R5 + ruling R4 WP2a:
            // 637.0 → 1875.0**, the measured un-lifted calculator feed on the
            // 2026-09-24 rows. The band moved with R5 to 0.063956-0.106593,
            // above the floor, so no floor acts. At 24 000 rpm x 2 flutes the
            // commanded advance is 1875 / 48 000 = 0.0391 mm/tooth, below
            // the 2500 mm/min machine ceiling.
            //
            // **RE-PINNED 2026-09-24, ruling R4 WP3: 1875.0 → 2500.0.** The
            // un-capped feed is 1875 / 0.75 = 2500, which is exactly the
            // 2500 mm/min machine ceiling, and equals the legacy shipped feed
            // that the same ceiling bound.
            //
            // **RE-PINNED 2026-09-24, ruling R4 Q10: 2500.0 → 2499.0.** The
            // raw feed sits on the 2500 mm/min ceiling (a hair above it in
            // floating point), so Step 7 now lowers the RPM to hold the chip
            // instead of clamping the feed. Chip per rev = 2500 / 24 000 =
            // 0.10417 mm; the RPM is floor(2500 / 0.10417) = 23 999 (one
            // whole rev under 24 000); the feed is 0.10417 x 23 999 =
            // 2499.896 mm/min, shipped floored to 2499.
            expected_suggest_feed: 2499.0,
        },
    ]
}

/// Two-op cascade: op 0 fresh-stock rough, op 1 the measured op reading what
/// op 0 left.
fn build_session(fx: &Fixture) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();

    let mut stock = stock_over(HALF, STOCK_Z);
    stock.material = fx.material.clone();
    builder = builder.stock(stock);

    builder = builder.machine(MachineProfile {
        max_feed_mm_min: fx.max_feed_mm_min,
        max_cutting_feed_mm_min: fx.max_cutting_feed_mm_min,
        ..MachineProfile::default()
    });

    let rt = builder.add_tool(fx.rough_tool.clone());
    let rough_id = builder.tools()[rt].id.0;
    let mt = builder.add_tool(fx.tool.clone());
    let meas_id = builder.tools()[mt].id.0;
    let model_id = builder.add_model(mesh_model(bumpy_surface(), "a5_bumps"));

    let heights = pinned_heights(STOCK_Z, STOCK_Z - RELIEF);

    let mut rough_op = adaptive3d_op(1500.0, 16_000, 2.5, 3.0);
    if let OperationConfig::Adaptive3d(cfg) = &mut rough_op {
        cfg.stock_to_leave_axial = ROUGH_STOCK_TO_LEAVE;
    }
    let mut rough = toolpath_config(
        "Rough (upstream, fresh stock)",
        rough_op,
        rough_id,
        model_id,
    );
    rough.heights = heights.clone();
    let _ = builder.add_toolpath(0, rough).expect("add rough op");

    let mut measured = toolpath_config(fx.label, fx.op.clone(), meas_id, model_id);
    measured.heights = heights;
    measured.stock_source = StockSource::FromRemainingStock;
    let _ = builder.add_toolpath(0, measured).expect("add measured op");
    builder.build()
}

// ── Suggest-side read ───────────────────────────────────────────────────

/// Everything the real Suggest path says about the measured op, before any
/// simulation. Field names use A-2's canonical vocabulary.
///
/// **Re-pointed 2026-08-13 (Checkpoint J-1).** Pre-fix this struct read
/// `requested_feed` / `raised_feed` / `lut_target` / `cap_hit` out of the
/// `FeedRaisedForChipload` warning, and `arc_fit_ratio` / `arc_fit_source`
/// off `predictions.observed_chipload`. All six sources were retired with
/// pass 8. What replaces them:
///
/// * `shipped_feed` — `suggested_operation.feed_rate()`, i.e. the number
///   the GUI writes. Post-retirement this IS the calculator's feed.
/// * `lut_target` — recomputed as the band **midpoint**, which is what
///   `SuggestAggressiveness::Default` aims at. Verified against the pre-fix
///   run: all four `lut_target` values the warning used to carry reproduce
///   as midpoints of the same bands to 5 dp (0.05400 / 0.06645 / 0.01739 /
///   0.01259).
/// * arms A and B — pinned on the fixture, see `Fixture::legacy_shipped_feed`.
#[derive(Debug, Clone)]
struct SuggestRead {
    /// `suggested_operation.feed_rate()` — the number the GUI writes.
    shipped_feed: f64,
    /// The band target arm C aims at: the derated band midpoint under the
    /// default aggressiveness policy.
    lut_target: f64,
    /// The derated vendor band on the matched row.
    band: Option<(f64, f64)>,
    /// The derated printed point on the matched row (A2, point mode):
    /// `FeedsResult::chipload_point_mm`. At most one of `band` and
    /// `point` is `Some`.
    point: Option<f64>,
    /// `feeds_result.rpm` — what the calculator recommended, for the record.
    feeds_rpm: f64,
    /// The RPM on the operation the fixture was *authored* with, kept only
    /// so the report can show that Suggest moved it.
    authored_rpm: u32,
    /// The machine's cutting-feed ceiling.
    cutting_ceiling: f64,
    flutes: u32,
    /// **The RPM Suggest resolved**, read off `suggested_operation` (falling
    /// back to `feeds_result.rpm`).
    ///
    /// This is not the RPM the fixture was authored with — an earlier
    /// Suggest pass rewrites `spindle_rpm`, and it differs from the authored
    /// value on three of the four fixtures (14 000 → 16 000, 18 000 →
    /// 19 000, 20 000 → 19 000). Deriving arm C from the authored RPM put
    /// DC-1's retirement ratio at 7.04× against a closed form of 6.67× —
    /// the harness's own first red, and the reason this field exists. It
    /// outlives pass 8 because every arm still runs at this RPM, which is
    /// what makes the A/B/C delta attributable to the feed alone.
    rpm: u32,
    /// Whether either retired chipload-lift warning appeared. Must be
    /// `false` — nothing in Suggest constructs them since 2026-08-13.
    any_retired_lift_warning: bool,
}

impl SuggestRead {
    /// Commanded advance per tooth (mm) at a given feed.
    fn commanded_fpt(&self, feed: f64) -> f64 {
        feed / (f64::from(self.rpm) * f64::from(self.flutes))
    }
    /// Arm C's feed: the solve aimed at the band target in the unit the gate
    /// now reports (i.e. the excluded "re-key the ratios to 1.0" reference).
    fn arm_c_feed(&self) -> f64 {
        (self.lut_target * f64::from(self.rpm) * f64::from(self.flutes)).min(self.cutting_ceiling)
    }
}

fn suggest_read(session: &ProjectSession, fx: &Fixture) -> SuggestRead {
    let tc = &session.toolpath_configs()[1];
    let profile = session
        .cutter_op_profile(tc)
        .expect("measured op resolves a tool");
    assert!(
        profile.feasibility.is_ok(),
        "{}: Suggest refused the tool × operation pairing: {:?}",
        fx.label,
        profile.feasibility
    );

    let feeds = profile
        .feeds
        .as_ref()
        .expect("feasibility Ok ⟹ feeds present");
    let suggested = profile
        .suggested_operation
        .as_ref()
        .expect("feasibility Ok ⟹ suggested operation present");

    let band = feeds
        .chipload_bounds
        .map(|b| (b.min_mm_per_tooth, b.max_mm_per_tooth));

    SuggestRead {
        shipped_feed: suggested.feed_rate(),
        // `SuggestAggressiveness::Default` (the policy `cutter_op_profile`
        // uses) targets the band midpoint.
        lut_target: band.map_or(f64::NAN, |(lo, hi)| (lo + hi) / 2.0),
        band,
        point: feeds.chipload_point_mm,
        feeds_rpm: feeds.rpm,
        authored_rpm: fx.rpm,
        cutting_ceiling: session.machine().cutting_feed_ceiling_mm_min(),
        flutes: fx.tool.flute_count,
        rpm: suggested
            .spindle_rpm()
            .unwrap_or_else(|| feeds.rpm.round() as u32),
        any_retired_lift_warning: profile.warnings.iter().any(|w| {
            matches!(
                w,
                SuggestWarning::FeedRaisedForChipload { .. }
                    | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
            )
        }),
    }
}

// ── Step 1–3: the Suggest-side before/after table ───────────────────────

/// **The disposition sentry — §6.1's `retired_lift_leaves_feed_at_the_
/// calculator_value`, red-first at `719b92d7`.**
///
/// Pre-fix this test was `arc_fit_lift_before_after_by_family` and it
/// *required* the lift to fire: it asserted `arc_fit_source == "Calibrated"`,
/// `raised_feed > requested_feed`, and — where no machine cap bound — that
/// `arm A / arm C == 1 / arc_fit_ratio` exactly. Those assertions measured
/// the defect and were **red-first inverted here**, not deleted: the closed
/// form they pinned (4.00× Adaptive3d, 6.67× DropCutter) is restated as the
/// arm-A/arm-C ratio the pinned constants still produce, so the numbers
/// remain checkable after the code that generated them is gone.
///
/// What it asserts now, per fixture:
///
/// 1. neither retired chipload-lift warning appears in `profile.warnings`;
/// 2. `suggested_operation.feed_rate()` equals the pinned arm-B feed
///    (4572.0 / 6000.0 / 660.0 / 2500.0, re-pinned 2026-09-24 for ruling R4
///    WP3) within
///    0.5 mm/min — Suggest ships the calculator's own number, re-derived at
///    the geometry the operation ships (G-SUGGEST-NOCLAMP, 2026-08-19), no
///    longer multiplied by chip thinning (G-CHIPTHIN-HALFFIX), and no longer
///    lifted to the rubbing floor (ruling R4 WP2a). On the 2026-09-24 rows the
///    two Adaptive3d fixtures resolve one-value rows (no band), DC-1 ships
///    under its band and warns, and DC-2 ships under its R5 band; the
///    per-fixture comments carry the arithmetic;
/// 3. the shipped feed is **at or below** the pinned legacy feed. Until
///    ruling R4 WP3 (2026-09-24) it was strictly below; with the 0.75 factor
///    gone, A3D-2 and DC-2 ship exactly the machine ceiling that also bound
///    the legacy lift (6000 and 2500 mm/min), so equality is the correct
///    reading there. A partial retirement still lands ABOVE neither and is
///    still caught on A3D-1 and DC-1, where no ceiling binds;
/// 4. the closed form still reproduces off the pinned constants where no
///    ceiling truncated the legacy lift (A3D-1 4.00×, DC-1 6.67×).
///
/// Point 3 is the one that would catch a *partial* retirement, and point 4
/// is what keeps this file's headline numbers falsifiable.
#[test]
fn retired_lift_leaves_feed_at_the_calculator_value() {
    println!("\n=== A-5i — the retirement, Suggest-side (no simulation) ===");
    println!(
        "{:<46} {:>12} {:>12} {:>10} {:>9} {:>9}",
        "fixture", "legacy_A", "shipped_B", "arm_C", "A/B", "A/C"
    );

    // Points 2 and 3 are a per-fixture TABLE, so they are collected and
    // reported together rather than aborting on the first row: failing fast
    // here hides the other three fixtures' measured feeds, which are the
    // numbers a reader needs in order to tell a real regression from a
    // deliberate re-baseline. (Changed 2026-08-19, when pass 9 moved two of
    // the four pins and the first row's abort concealed the rest.)
    let mut mismatches: Vec<String> = Vec::new();

    for fx in fixtures() {
        let session = build_session(&fx);
        let r = suggest_read(&session, &fx);

        let arm_c = r.arm_c_feed();
        let a_over_b = fx.legacy_shipped_feed / r.shipped_feed;
        let a_over_c = fx.legacy_shipped_feed / arm_c;

        println!(
            "{:<46} {:>12.1} {:>12.1} {:>10.1} {:>8.2}× {:>8.2}×",
            fx.label, fx.legacy_shipped_feed, r.shipped_feed, arm_c, a_over_b, a_over_c,
        );
        println!(
            "      band {:?}  target {:.5}  ceiling {:.0}  \
             rpm solved-against {} (authored {}, feeds.rpm {:.0})  flutes {}",
            r.band, r.lut_target, r.cutting_ceiling, r.rpm, r.authored_rpm, r.feeds_rpm, r.flutes
        );
        println!(
            "      commanded advance/tooth  legacy {:.5}  shipped {:.5} mm  (band max {:.5})",
            r.commanded_fpt(fx.legacy_shipped_feed),
            r.commanded_fpt(r.shipped_feed),
            r.band.map_or(f64::NAN, |b| b.1),
        );

        // 1 — the lift is gone from the warning stream.
        assert!(
            !r.any_retired_lift_warning,
            "{}: no Suggest pass has constructed FeedRaisedForChipload or \
             ChiploadStillLowAfterRecalibration since 2026-08-13 (Checkpoint J-1/J-5)",
            fx.label
        );

        // 2 — Suggest ships the calculator's feed.
        if (r.shipped_feed - fx.expected_suggest_feed).abs() >= 0.5 {
            mismatches.push(format!(
                "{}: Suggest must ship the un-lifted feed {} ± 0.5, got {}",
                fx.label, fx.expected_suggest_feed, r.shipped_feed
            ));
        }

        // 3 — and that is no more than what it used to ship. Where the
        //     cutting ceiling bound both (A3D-2, DC-2) the two are equal
        //     since ruling R4 WP3; elsewhere the shipped feed is strictly
        //     below, and a partial retirement would land between the two.
        // "On the ceiling" is within 1.5 mm/min under it since ruling R4
        // Q10: the whole-rev RPM floor and the feed floor put a ceiling-bound
        // feed up to about one mm/min under the ceiling (DC-2: 2499 on 2500).
        let ceiling_bound = r.cutting_ceiling - r.shipped_feed < 1.5;
        if r.shipped_feed > fx.legacy_shipped_feed + 0.5
            || (!ceiling_bound && r.shipped_feed >= fx.legacy_shipped_feed - 0.5)
        {
            mismatches.push(format!(
                "{}: shipped feed {} must sit at or below the retired lift's {} (strictly \
                 below unless the cutting ceiling {} binds both)",
                fx.label, r.shipped_feed, fx.legacy_shipped_feed, r.cutting_ceiling
            ));
        }

        // 4 — the closed form, restated on the pinned constants. It holds
        //     only where the machine cutting-feed ceiling did not truncate
        //     the legacy lift; A3D-2 and DC-2 were capped and are excluded
        //     by that condition, exactly as pre-fix.
        //
        // **A-7 / Checkpoint K-(a4), 2026-08-13 — the Adaptive3d arm of
        // this identity is RETIRED, not re-pinned.** `arm_C` re-keys the
        // feed to the matched band, and a4 changed which band an
        // Adaptive3d query matches (Adaptive → Pocket). The identity
        // `legacy_A / arm_C == 1/arc_fit_ratio` was a statement about the
        // retired table's ratio *against the Adaptive band it was derived
        // from*; with a different band underneath it, A3D-1 measures
        // **5.297** where it measured 4.000. Re-pinning 4.0 → 5.297 would
        // assert a coincidence rather than the closed form, so the check
        // is scoped to the family a4 did not move (DropCutter, unchanged
        // at 6.67), and the Adaptive3d arm is replaced below by the
        // property that IS now true: the band it re-keys against is the
        // Pocket row's.
        let uncapped = fx.legacy_shipped_feed < r.cutting_ceiling - 0.5;
        //
        // **2026-09-23, feeds matrix R5.** The R5 printed Spektra rows
        // (`amana-flat-hardwood-pocket-6000-2f-spektra`, max 0.127, no
        // minimum) now win the Ø6 hardwood query. The chart-display ruling
        // (5199e06e) gives a one-value row no band, so A3D-1 carries
        // `band == None` and has no arm C. The a4 check therefore reads: a
        // band, when one resolves, is never the Adaptive row's (max ≈ 0.070).
        if fx.family == "Adaptive3d" {
            match r.band {
                None => println!(
                    "      {}: no two-limit band resolves (a one-value row); arm C \
                     and the band checks do not apply",
                    fx.label
                ),
                Some((_, band_max)) if (band_max - 0.070).abs() < 0.005 => {
                    mismatches.push(format!(
                        "{}: an Adaptive3d query resolved the ADAPTIVE band (max \
                         {band_max:.5}); K-(a4) routes it to the Pocket family",
                        fx.label
                    ));
                }
                Some(_) => {}
            }
            continue;
        }
        let nominal = 1.0 / 0.15;
        if uncapped {
            assert!(
                (a_over_c - nominal).abs() < 0.02,
                "{}: uncapped, legacy A / arm C must equal 1/arc_fit_ratio = {:.3} \
                 (the retired table's closed form), measured {:.3}",
                fx.label,
                nominal,
                a_over_c
            );
        }
    }

    assert!(
        mismatches.is_empty(),
        "the Suggest-side disposition table moved:\n  {}",
        mismatches.join("\n  ")
    );
}

// ── Sim-side arms ───────────────────────────────────────────────────────

/// One arm's post-simulation reading of the measured op.
#[derive(Debug)]
struct ArmRead {
    applied_feed: f64,
    /// Provenance of the machined stock this generation consumed.
    stamp: Option<StockSnapshotStamp>,
    verdict: String,
    /// Stage 1 — commanded advance per tooth (mm).
    commanded_fpt: Option<f64>,
    /// Stage 2 — the vendor band.
    band: Option<(Option<f64>, f64)>,
    /// Stage 3 — achieved / commanded feed, and whether it was measured.
    achieved_ratio: Option<f64>,
    predicted_feeds_present: bool,
    /// Stage 4 — the gate's own observation and its population.
    gate_observed: Option<f64>,
    gate_samples: usize,
    /// Post-sim modulation, when it ran.
    modulation: Option<(usize, usize, f64)>,
}

fn simulate(session: &mut ProjectSession, modulation: bool) {
    let cancel = AtomicBool::new(false);
    // G-RESTRES: the project stores the ONE cell every simulation reads.
    let _ = session
        .apply(rs_cam_core::session::Command::SetSimulationResolution(
            rs_cam_core::session::SetSimulationResolutionArgs {
                resolution: rs_cam_core::session::SimulationResolution::Fixed(SIM_CELL_MM),
            },
        ))
        .expect("a positive cell");
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SIM_CELL_MM,
                auto_resolution: false,
                metrics_enabled: true,
                adaptive_feed_modulation: modulation,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("cascade simulation");
}

/// Run one arm: write `feed` onto the measured op, regenerate it against the
/// stock in `prior_stocks`, simulate, and read every stage.
///
/// **WP7 — the write is a command, so the arm re-seeds.** Every command
/// row that moves a generation input calls `invalidate_output_dependents`,
/// which clears `session.simulation`, and `prior_stocks` lives there. The
/// measured op REFUSES to generate without a snapshot, so the arm
/// simulates the rough again between the write and the regeneration. The
/// re-seed reads index 0's cached result, which the write leaves alone —
/// the chain walk goes downstream of index 1 — so every arm still consumes
/// the same snapshot. `arc_fit_arms_gate_observation` asserts exactly that
/// through `ArmRead::stamp`.
fn run_arm(session: &mut ProjectSession, feed: f64, rpm: u32, modulation: bool) -> ArmRead {
    {
        let mut config = session
            .get_toolpath_config(1)
            .expect("measured op present")
            .clone();
        config.operation.set_feed_rate(feed);
        config.operation.set_spindle_rpm(Some(rpm));
        let _ = session
            .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index: 1,
                config: Box::new(config),
            }))
            .expect("measured op present");
    }
    simulate(session, false);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(1, &cancel)
        .expect("measured op generates");

    let stamp = session
        .get_result(1)
        .expect("measured op result")
        .stats
        .stock_snapshot;

    simulate(session, modulation);

    let tp_id = session.toolpath_configs()[1].id;
    let report = session.tool_load_report();
    let v = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == tp_id)
        .expect("measured op has a load verdict");

    let fe = v.feed_explanation.as_deref();
    ArmRead {
        applied_feed: feed,
        stamp,
        verdict: format!("{:?}", v.chipload.state()),
        commanded_fpt: fe.map(|e| e.commanded.feed_per_tooth_mm),
        band: fe.map(|e| (e.band.min_mm_per_tooth, e.band.max_mm_per_tooth)),
        achieved_ratio: fe.and_then(|e| e.achieved_feed.median_ratio),
        predicted_feeds_present: fe.is_some_and(|e| e.achieved_feed.predicted_feeds_present),
        gate_observed: fe.map(|e| e.gate.value_mm),
        gate_samples: fe.map_or(0, |e| e.gate.sample_count),
        modulation: v
            .modulation_summary
            .as_ref()
            .map(|m| (m.moves_touched, m.moves_total, m.median_feed_delta_pct)),
    }
}

fn print_arm(tag: &str, a: &ArmRead) {
    println!(
        "  {tag:<22} feed {:>8.1}  verdict {:<10} cmd_fpt {:>9}  achieved× {:>8}  \
         gate {:>9} (n={})",
        a.applied_feed,
        a.verdict,
        a.commanded_fpt
            .map_or("—".to_owned(), |v| format!("{v:.5}")),
        a.achieved_ratio
            .map_or("—".to_owned(), |v| format!("{v:.4}")),
        a.gate_observed
            .map_or("—".to_owned(), |v| format!("{v:.5}")),
        a.gate_samples,
    );
    println!(
        "  {:<22} band {:?}  predicted_feeds {}  modulation {:?}  stamp {:?}",
        "",
        a.band,
        a.predicted_feeds_present,
        a.modulation,
        a.stamp.map(|s| (s.cell_size_mm(), s.digest)),
    );
}

/// **The J-3 default-flip sentry — red-first at the retirement commit.**
///
/// Checkpoint J-3 (operator, BINDING, 2026-08-13) flipped
/// `SimulationOptions::default().adaptive_feed_modulation` from `false` to
/// `true`. This asserts the flip and, in the same test, the reason for it.
///
/// Three assertions, all RED before the flip:
///
/// 1. the default is `true` — fails outright at the retirement commit;
/// 2. under a **default-constructed** `SimulationOptions` (the field
///    deliberately NOT named), Suggest's shipped feed lands `Within` on
///    **all four** fixtures — pre-flip the two DropCutter fixtures read
///    `Exceeds` at 1.69× / 1.62× the band max;
/// 3. the modulation actually ran — `modulation_summary` is `Some` with
///    moves touched. Without this, assertion 2 could pass vacuously on a
///    fixture the modulator silently skipped.
///
/// This is the 2/4 → 4/4 step. Retiring the lift (J-1) got Suggest's
/// recommendation from `Exceeds` 4/4 to `Exceeds` 2/4; the residual two are
/// the DOC-derate denominator divergence (census F-3 / C-2 / C-5), which
/// **arc-fit was amplifying ~3× but did not cause**. Modulation is what
/// closes them, because it corrects from a measured observation instead of
/// an operation-family constant — which is exactly disposition (c), and the
/// reason J-1 and J-3 are one ruling in two commits.
///
/// Note the population: assertion 3 guards against the vacuous-gate failure
/// mode (X-VAC) on the *modulator* rather than the gate. A modulator that
/// no-oped would leave the commanded feed in place, and on A3D-1/A3D-2 that
/// still reads `Within` — the test would go green for the wrong reason.
#[test]
fn modulation_default_is_on_and_closes_the_two_dropcutter_residuals() {
    println!("\n=== A-5i — Checkpoint J-3, the default flip (cell {SIM_CELL_MM} mm) ===");

    // 1 — the flip itself.
    assert!(
        SimulationOptions::default().adaptive_feed_modulation,
        "Checkpoint J-3: SimulationOptions::default() must have adaptive_feed_modulation = true"
    );

    // Non-vacuity for the target-less skip below. A2 (point mode): the
    // modulator caps a point toolpath at its printed point, so a fixture
    // with a band OR a point is a modulation target. The arm needs at
    // least one modulated fixture of each kind.
    let mut targeted = 0usize;
    let mut modulated = 0usize;
    let mut modulated_bands = 0usize;
    let mut modulated_points = 0usize;
    for fx in fixtures() {
        let mut session = build_session(&fx);
        let r = suggest_read(&session, &fx);

        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .expect("rough generates");

        // The measured arm: Suggest's shipped feed, simulated under options
        // that DO NOT name `adaptive_feed_modulation`. That omission is the
        // point — this arm reads the default.
        //
        // WP7: the write comes BEFORE the seeding simulation now. The
        // command row clears `session.simulation`, and the seed is what
        // puts the snapshot there. At this point the measured op carries no
        // result, so the row drops nothing.
        {
            let mut config = session
                .get_toolpath_config(1)
                .expect("measured op present")
                .clone();
            config.operation.set_feed_rate(r.shipped_feed);
            config.operation.set_spindle_rpm(Some(r.rpm));
            let _ = session
                .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                    index: 1,
                    config: Box::new(config),
                }))
                .expect("measured op present");
        }
        // Seed the cascade unmodulated so the snapshot the measured op reads
        // is the same one every other test in this file measures against.
        simulate(&mut session, false);
        session
            .generate_toolpath(1, &cancel)
            .expect("measured op generates");
        // G-RESTRES: the project stores the ONE cell every simulation reads.
        let _ = session
            .apply(rs_cam_core::session::Command::SetSimulationResolution(
                rs_cam_core::session::SetSimulationResolutionArgs {
                    resolution: rs_cam_core::session::SimulationResolution::Fixed(SIM_CELL_MM),
                },
            ))
            .expect("a positive cell");
        session
            .run_simulation(
                &SimulationOptions {
                    resolution: SIM_CELL_MM,
                    auto_resolution: false,
                    metrics_enabled: true,
                    // `adaptive_feed_modulation` deliberately unset.
                    ..SimulationOptions::default()
                },
                &cancel,
            )
            .expect("default-options simulation");

        let tp_id = session.toolpath_configs()[1].id;
        let report = session.tool_load_report();
        let v = report
            .per_toolpath
            .iter()
            .find(|v| v.toolpath_id == tp_id)
            .expect("measured op has a load verdict");
        let fe = v.feed_explanation.as_deref();
        let verdict = format!("{:?}", v.chipload.state());
        let observed = fe.map(|e| e.gate.value_mm);
        let samples = fe.map_or(0, |e| e.gate.sample_count);

        println!(
            "  {:<46} feed {:>8.1}  verdict {:<10} gate {:>9} (n={})  modulation {:?}",
            fx.label,
            r.shipped_feed,
            verdict,
            observed.map_or("—".to_owned(), |v| format!("{v:.5}")),
            samples,
            v.modulation_summary.as_ref().map(|m| (
                m.moves_touched,
                m.moves_total,
                m.median_feed_delta_pct
            )),
        );

        // A2 (point mode, 2026-09-24): a fixture whose row prints one value
        // modulates capped at the point, so 2 and 3 apply to it. Only a
        // fixture with no chipload row at all (no band and no point) has
        // nothing for the modulator to aim at.
        if r.band.is_none() && r.point.is_none() {
            println!(
                "      {}: no band and no point; modulation checks skipped",
                fx.label
            );
            continue;
        }
        targeted += 1;

        // Ruling R4 WP3 (2026-09-24): a fixture whose shipped feed sits ON
        // its cutting ceiling gives the modulator nothing to move (DC-2 ships
        // exactly 2500 mm/min on a 2500 mm/min router). Skip it and say so;
        // DC-1 is the fixture that must modulate.
        // Within 1.5 mm/min under the ceiling (ruling R4 Q10: DC-2 ships 2499
        // on its 2500 router), the modulator still has nothing to move.
        if r.cutting_ceiling - r.shipped_feed < 1.5 {
            println!(
                "      {}: shipped feed {:.1} sits on the {:.1} mm/min ceiling; \
                 modulation checks skipped",
                fx.label, r.shipped_feed, r.cutting_ceiling
            );
            continue;
        }
        modulated += 1;
        if r.band.is_some() {
            modulated_bands += 1;
        } else {
            modulated_points += 1;
        }
        println!(
            "      {}: modulation target {}",
            fx.label,
            match (r.band, r.point) {
                (Some((lo, hi)), _) => format!("band {lo:.5}-{hi:.5}"),
                (None, Some(v)) => format!("point {v:.5}"),
                (None, None) => "none".to_owned(),
            }
        );

        // 3 — the modulator ran, and on a non-empty population. Checked
        //     BEFORE the verdict so a vacuous pass cannot be read as a fix.
        let summary = v.modulation_summary.as_ref().unwrap_or_else(|| {
            panic!(
                "{}: the default must actually modulate — no modulation_summary means the \
                 verdict below would be the unmodulated one wearing the flip's name",
                fx.label
            )
        });
        assert!(
            summary.moves_touched > 0,
            "{}: modulation touched 0 of {} moves — vacuous",
            fx.label,
            summary.moves_total
        );
        assert!(
            samples > 0,
            "{}: chipload gate ran on an EMPTY population (X-VAC)",
            fx.label
        );

        // 2 — and the verdict is clean on all four, including the two
        //     DropCutter fixtures that are Exceeds without modulation.
        assert_eq!(
            verdict, "Within",
            "{}: under the J-3 default, Suggest's shipped feed must land inside the \
             chipload band. Unmodulated this fixture reads {} (the DropCutter pair \
             are Exceeds at 1.69× / 1.62× band max — census F-3 / C-2 / C-5).",
            fx.label, verdict
        );
    }
    // DC-1 carries a two-limit band on the 2026-09-24 rows; A3D-1 and A3D-2
    // resolve one-value rows (points, A2). A3D-2 and DC-2 ship on their
    // cutting ceilings, so DC-1 (band) and A3D-1 (point) must modulate.
    assert!(
        targeted >= 2,
        "only {targeted} fixtures carried a band or a point; the modulation arm is vacuous"
    );
    assert!(
        modulated >= 1,
        "no targeted fixture sat off its cutting ceiling, so nothing was checked to \
         modulate; DC-1 and A3D-1 must"
    );
    assert!(
        modulated_bands >= 1,
        "no two-limit band fixture was checked to modulate; DC-1 must"
    );
    assert!(
        modulated_points >= 1,
        "no point fixture was checked to modulate (A2); A3D-1 must"
    );
}

/// **Step 1–2: Suggest → generate → simulate, three arms, same stock.**
///
/// The arms differ in commanded feed only. The stamp assertion is what makes
/// that claim checkable rather than assumed.
#[test]
fn arc_fit_arms_gate_observation() {
    println!("\n=== A-5 step 1/2 — simulation-backed arms (cell {SIM_CELL_MM} mm) ===");

    // Non-vacuity for the band-less skip of the pre-fix reproduction.
    let mut reproduced = 0usize;
    for (fx_index, fx) in fixtures().into_iter().enumerate() {
        let mut session = build_session(&fx);
        let r = suggest_read(&session, &fx);

        // Generate the rough. The rest op REFUSES to generate before a
        // snapshot exists (it will not fall back to fresh stock), so a
        // simulation has to come first — and since WP7 each arm seeds that
        // snapshot itself, after its own command write clears it. A seed
        // here would be dropped by the first arm's write.
        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .expect("rough generates");

        println!("\n{} [{}]", fx.label, fx.family);

        // Every arm runs at the RPM Suggest resolved, so the A/B/C delta is
        // attributable to the commanded feed alone.
        //
        // Arm A is the PINNED legacy feed (see `Fixture::legacy_shipped_feed`)
        // — post-retirement Suggest no longer produces it, and the pre-fix
        // reproduction below must survive the fix. Arm B is what Suggest
        // ships today; `retired_lift_leaves_feed_at_the_calculator_value`
        // asserts that is the same number arm B carried pre-fix.
        let arm_a = run_arm(&mut session, fx.legacy_shipped_feed, r.rpm, false);
        let arm_b = run_arm(&mut session, r.shipped_feed, r.rpm, false);
        // 2026-09-23 (feeds matrix R5): arm C aims at the band midpoint, so a
        // fixture with a one-value row (A3D-1 since the Spektra rows) has no
        // arm C. It still runs arms A and B.
        let arm_c = r
            .band
            .is_some()
            .then(|| run_arm(&mut session, r.arm_c_feed(), r.rpm, false));
        let arm_a_mod = run_arm(&mut session, fx.legacy_shipped_feed, r.rpm, true);
        let arm_b_mod = run_arm(&mut session, r.shipped_feed, r.rpm, true);

        print_arm("A shipped", &arm_a);
        print_arm("B retire", &arm_b);
        if let Some(arm_c) = &arm_c {
            print_arm("C re-key 1.0", arm_c);
        }
        print_arm("A + modulation", &arm_a_mod);
        print_arm("B + modulation", &arm_b_mod);

        // ── S-4 handoff: the arms must have consumed the same stock ────
        for (tag, arm) in [
            ("B", Some(&arm_b)),
            ("C", arm_c.as_ref()),
            ("A+mod", Some(&arm_a_mod)),
            ("B+mod", Some(&arm_b_mod)),
        ]
        .into_iter()
        .filter_map(|(tag, arm)| arm.map(|a| (tag, a)))
        {
            assert_eq!(
                arm.stamp, arm_a.stamp,
                "{}: arm {tag} consumed a different machined-stock snapshot than arm A — \
                 the comparison is not fair (§6.1 rule 6). A={:?} {tag}={:?}",
                fx.label, arm_a.stamp, arm.stamp
            );
        }
        let stamp_a = arm_a.stamp.unwrap_or_else(|| {
            panic!(
                "{}: the cascade must give the measured op a machined-stock snapshot, \
                 otherwise the equality above is vacuous",
                fx.label
            )
        });
        // The stamp must be reading THIS run's grid, not a constant. Equal
        // digests across fixtures are expected here — every fixture's op 0 is
        // the same rough on the same stock — so the cell is what proves the
        // channel is live per-run.
        assert!(
            (stamp_a.cell_size_mm() - SIM_CELL_MM).abs() < 1e-12,
            "{}: stamp cell {} must be the cell this harness simulated at ({SIM_CELL_MM})",
            fx.label,
            stamp_a.cell_size_mm()
        );

        // ── Non-vacuity: the gate must have had a population ───────────
        assert!(
            arm_a.gate_samples > 0,
            "{}: arm A's chipload gate ran on an EMPTY population — a verdict from \
             an empty gate is vacuous (X-VAC)",
            fx.label
        );

        // ── The pre-fix reproduction (§0 rule 1 — stays permanently) ───
        //
        // Suggest's OWN recommendation, applied unmodified and simulated,
        // fails the chipload gate Suggest claims to target. Arm B (the lift
        // removed) and arm C (the lift re-aimed at the gate's actual unit)
        // do not. This is the finding; it must fail loudly if it ever
        // silently stops being true.
        //
        // 2026-09-24 (feeds matrix R5): the reproduction is observable only
        // where the LEGACY advance per tooth sits above the band maximum
        // Suggest resolves; the condition is computed from the legacy feed,
        // the solved RPM and the flutes, not from a fixed list. On the
        // 2026-09-24 rows only DC-1 reproduces: 4405 / (19 000 x 2) = 0.116
        // mm/tooth, above the 0.0232 maximum. A3D-1 and A3D-2 resolve
        // one-value printed rows (the Spektra rows) and carry no band. DC-2's
        // band moved with R5 to about 0.064-0.107, and its legacy
        // 2500 / (24 000 x 2) = 0.052 sits under it. For those three cells the
        // disposition evidence is historical (the 2026-08-13 record above);
        // they print that the check was skipped.
        let legacy_fpt = r.commanded_fpt(fx.legacy_shipped_feed);
        if r.band.is_some_and(|(_, band_max)| legacy_fpt > band_max) {
            reproduced += 1;
            assert_eq!(
                arm_a.verdict, "Exceeds",
                "{}: arm A is the pre-fix reproduction — Suggest's shipped feed must \
                 trip the chipload gate. If this changed, the disposition evidence \
                 is stale and Checkpoint J must be re-run.",
                fx.label
            );
        } else {
            println!(
                "      {}: legacy advance {legacy_fpt:.5} is not above a two-limit band \
                 maximum; the pre-fix reproduction is historical on this cell and was \
                 skipped (arm A read {})",
                fx.label, arm_a.verdict
            );
        }
        if let Some(arm_c) = &arm_c {
            assert_eq!(
                arm_c.verdict, "Within",
                "{}: arm C aims the same solve at the unit the gate reports and must \
                 land inside the band",
                fx.label
            );
        }

        // ── Modulation erases the lift entirely ────────────────────────
        //
        // A and B are commanded 2.4×–5.7× apart, yet under adaptive feed
        // modulation both converge on the SAME gate observation. The lift's
        // only effect in a modulated workflow is how far the modulator has
        // to travel to undo it.
        //
        // A fixture with a one-value row (A3D-1, A3D-2 on the 2026-09-24
        // rows) gives the modulator no band to converge into: it touched 0
        // moves on both arms. The check applies to DC-1 and DC-2 only.
        let converges = r.band.is_some();
        if !converges {
            println!(
                "      {}: no two-limit band; convergence check skipped",
                fx.label
            );
        }
        let (Some(obs_a), Some(obs_b)) = (arm_a_mod.gate_observed, arm_b_mod.gate_observed) else {
            panic!(
                "{}: both modulated arms must produce a gate observation",
                fx.label
            )
        };
        assert!(
            !converges || (obs_a - obs_b).abs() <= 1e-6 * obs_a.abs().max(1.0),
            "{}: modulated arms A and B must converge on one operating point, \
             got {obs_a} vs {obs_b}",
            fx.label
        );

        // ── Instrument responsiveness, once ────────────────────────────
        //
        // Every stamp above compared EQUAL. A channel that can only ever
        // return equal has proven nothing, so on the first fixture only,
        // move the snapshot and require the stamp to notice (S-4 arm B2's
        // result, re-checked here because this file leans on it).
        if fx_index == 0 {
            let cancel = AtomicBool::new(false);
            // G-RESTRES: the project stores the ONE cell every simulation reads.
            let _ = session
                .apply(rs_cam_core::session::Command::SetSimulationResolution(
                    rs_cam_core::session::SetSimulationResolutionArgs {
                        resolution: rs_cam_core::session::SimulationResolution::Fixed(
                            SIM_CELL_MM * 2.0,
                        ),
                    },
                ))
                .expect("a positive cell");
            session
                .run_simulation(
                    &SimulationOptions {
                        resolution: SIM_CELL_MM * 2.0,
                        auto_resolution: false,
                        metrics_enabled: false,
                        ..SimulationOptions::default()
                    },
                    &cancel,
                )
                .expect("re-simulate at a different cell");
            session
                .generate_toolpath(1, &cancel)
                .expect("regenerate against the moved snapshot");
            let moved = session
                .get_result(1)
                .expect("measured op result")
                .stats
                .stock_snapshot
                .expect("still a machined-stock generation");
            println!(
                "  {:<22} stamp {:?} (was {:?})",
                "stamp responsiveness",
                (moved.cell_size_mm(), moved.digest),
                (stamp_a.cell_size_mm(), stamp_a.digest),
            );
            assert_ne!(
                moved, stamp_a,
                "{}: re-simulating at a different cell must move the stamp — \
                 otherwise the equality assertions above are vacuous",
                fx.label
            );
        }
    }
    assert!(
        reproduced >= 1,
        "the pre-fix reproduction ran on no fixture; DC-1 (legacy 0.116 mm/tooth over a \
         0.0232 band maximum) must carry it"
    );
}
