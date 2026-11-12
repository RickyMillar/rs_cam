//! Feeds contract P21c (`planning/feeds_contract_2026-11-03/phases/P21c.md`,
//! `P21_DESIGN.md` section 2): under `SpindleStrategy::ConstantChip` the
//! chip per tooth holds and the RPM rises to the first named limit
//! (`feeds::RpmLimit`). The feed scales with the RPM. No multiplier enters.
//!
//! The machine is "wanaka-like": the Shapeoko VFD preset (6000 to 24000
//! RPM, 1.5 kW constant torque, rated RPM 24000) with the travel rate, the
//! axis rates and the accelerations that
//! `planning/rivmap_block_job/nz_wanaka_21_operator.toml` prints
//! (`[job.machine]`, `[job.machine.kinematics]`). The tool is a 6 mm
//! 2-flute flat end mill in white oak, the tool and wood of that project.
//!
//! The arms:
//!
//! - (a) a side op with a ceiling of 24000 ships 24000, the spindle
//!   maximum; the chip per rev is the `MatchChart` chip per rev; the power
//!   admits every RPM (rated RPM = spindle maximum);
//! - (b) a drill ships `floor(Z rate / chip per rev)`, the Z axis rate
//!   (`P21_DESIGN.md` section 2.3: 2500 / (0.0635 x 2) = 19685);
//! - (c) a small X/Y rate binds `CuttingCeiling(AxisRate)`; an op feed cap
//!   binds `CuttingCeiling(OpFeedCap)` through the same ceiling (ruling
//!   Q11), and Step 7 reads that ceiling under `MatchChart` too;
//! - (d) a `ConstantPower` machine binds `PowerAtRpm` at `P / k`, with `k`
//!   the required power per RPM of the chart run;
//! - (e) a constant-torque drive with a rated RPM below the spindle maximum
//!   binds `PowerAtRpm` at `n_r / R`;
//! - (f) a ceiling below the other limits binds `OperatorCeiling`; a
//!   ceiling below the chart RPM lowers the RPM to the ceiling;
//! - (g) an empty ceiling ships the chart RPM and feed;
//! - (h) a row that prints no RPM takes the lowest limit when the operator
//!   states a ceiling; the surface-speed RPM is not read (ruling Q6);
//! - (i) an 18 mm 2-flute flat tool: the probe that the phase note records.
//!
//! Every expected value comes from the machine values above or from the
//! `MatchChart` run of the same cell.
//!
//! Red: remove the `ZAxisRate` limit from `calculator/spindle.rs::named_limits`
//! (arm b), or read `cutting_feed_ceiling_mm_min()` there (arm c).

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::suggest::{
    SuggestContext, SuggestForOperationInput, suggest_for_operation,
};
use rs_cam_core::feeds::{
    FeedsResult, RpmChoice, RpmLimit, SpindleScaleReason, SpindleStrategy, calculate,
    embedded_vendor_lut,
};
use rs_cam_core::machine::{CuttingCeilingSource, MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

/// `[job.machine] max_feed_mm_min` and the X and Y of `max_rate_xyz_mm_min`.
const WANAKA_XY_MM_MIN: f64 = 15_000.0;
/// The Z of `max_rate_xyz_mm_min` (`$112`).
const WANAKA_Z_MM_MIN: f64 = 2500.0;
/// `acceleration_xyz_mm_s2`.
const WANAKA_ACCEL_MM_S2: [f64; 3] = [1200.0, 1200.0, 550.0];
/// `[job.post] spindle_rpm_ceiling` of the wanaka file.
const CEILING: u32 = 24_000;

fn wanaka_like() -> MachineProfile {
    with_rates(WANAKA_XY_MM_MIN)
}

/// The wanaka-like machine with the X and Y rate `xy` (the travel rate
/// holds).
fn with_rates(xy: f64) -> MachineProfile {
    MachineProfile {
        max_feed_mm_min: WANAKA_XY_MM_MIN,
        ..MachineProfile::shapeoko_vfd()
    }
    .with_axis_limits([xy, xy, WANAKA_Z_MM_MIN], WANAKA_ACCEL_MM_S2)
}

fn flat_2f(diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(4.0 * diameter));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(32.0));
    t
}

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn constant_chip(rpm_ceiling: Option<u32>) -> SpindleStrategy {
    SpindleStrategy::ConstantChip { rpm_ceiling }
}

/// The calculator result of the production Suggest door.
fn run(
    op: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    strategy: SpindleStrategy,
) -> FeedsResult {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool,
        machine,
        material: &white_oak(),
        lut: embedded_vendor_lut(),
        spindle_strategy: strategy,
        context: SuggestContext::default(),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
    .feeds_result
}

fn choice(r: &FeedsResult) -> &RpmChoice {
    r.rpm_choice.as_ref().expect("ConstantChip gives a choice")
}

fn limit_rpm(r: &FeedsResult, limit: RpmLimit) -> f64 {
    choice(r)
        .limits
        .iter()
        .find(|l| l.limit == limit)
        .unwrap_or_else(|| panic!("no {limit:?} in {:?}", choice(r).limits))
        .rpm
}

/// The chip per rev (mm): the feed over the RPM.
fn chip_per_rev(r: &FeedsResult) -> f64 {
    r.feed_rate_mm_min / r.rpm
}

fn pocket() -> OperationConfig {
    fixture_op(OperationType::Pocket)
}

/// (a) A side op rises to the spindle maximum and holds the chip.
#[test]
fn a_side_op_rises_to_the_spindle_maximum_p21() {
    let (op, tool, machine) = (pocket(), flat_2f(6.0), wanaka_like());
    let chart = run(&op, &tool, &machine, SpindleStrategy::MatchChart);
    assert!(chart.rpm_choice.is_none(), "MatchChart has no choice");
    let (_, spindle_max) = machine.rpm_range();
    assert!(
        chart.rpm < spindle_max,
        "non-vacuity: the chart RPM {} is below the spindle maximum",
        chart.rpm
    );

    let r = run(&op, &tool, &machine, constant_chip(Some(CEILING)));
    assert_eq!(r.rpm, spindle_max);
    let c = choice(&r);
    assert_eq!(c.rpm, spindle_max);
    assert_eq!(c.chart_rpm, Some(chart.rpm));
    // The spindle maximum and the ceiling tie at 24000; the first limit of
    // the list is the spindle maximum.
    assert_eq!(c.binding, RpmLimit::SpindleMax);
    assert_eq!(
        r.derates.spindle_scale_reason,
        SpindleScaleReason::ConstantChip
    );
    assert!(
        (r.derates.spindle_scale - spindle_max / chart.rpm).abs() < 1e-12,
        "the scale is the RPM over the chart RPM, no multiplier: {}",
        r.derates.spindle_scale
    );
    assert!(
        (chip_per_rev(&r) - chip_per_rev(&chart)).abs() < 1e-9,
        "the chip holds: {} vs {}",
        chip_per_rev(&r),
        chip_per_rev(&chart)
    );
    // The XY limit is the cutting ceiling over the chip per rev.
    let n_xy = limit_rpm(&r, RpmLimit::CuttingCeiling(CuttingCeilingSource::AxisRate));
    assert!(
        (n_xy - WANAKA_XY_MM_MIN / chip_per_rev(&chart)).abs() < 1e-6,
        "n_xy {n_xy}"
    );
    // Rated RPM = spindle maximum: the power admits every RPM.
    assert!(
        limit_rpm(&r, RpmLimit::PowerAtRpm).is_infinite(),
        "a constant-torque drive below its rated RPM sets no RPM"
    );
    // A side op takes no Z limit.
    assert!(
        !c.limits.iter().any(|l| l.limit == RpmLimit::ZAxisRate),
        "a side op has no Z limit: {:?}",
        c.limits
    );
}

/// (b) A drill rises to the Z axis rate.
#[test]
fn a_drill_rises_to_the_z_axis_rate_p21() {
    let op = OperationConfig::new_default(OperationType::Drill);
    let (tool, machine) = (flat_2f(6.0), wanaka_like());
    let chart = run(&op, &tool, &machine, SpindleStrategy::MatchChart);
    assert!(
        chart.feed_rate_mm_min < WANAKA_Z_MM_MIN,
        "non-vacuity: the chart drill feed {} is below the Z rate",
        chart.feed_rate_mm_min
    );
    let want = (WANAKA_Z_MM_MIN / chip_per_rev(&chart)).floor();

    let r = run(&op, &tool, &machine, constant_chip(Some(CEILING)));
    let c = choice(&r);
    assert_eq!(c.binding, RpmLimit::ZAxisRate, "{:?}", c.limits);
    assert_eq!(c.rpm, want);
    assert_eq!(r.rpm, want, "no later step lowers the drill RPM");
    assert!(
        r.feed_rate_mm_min <= WANAKA_Z_MM_MIN,
        "the drill feed {} is at or below the Z rate",
        r.feed_rate_mm_min
    );
    assert!(
        (chip_per_rev(&r) - chip_per_rev(&chart)).abs() < 1e-9,
        "the chip holds"
    );
    // `P21_DESIGN.md` section 2.3: 2500 / (0.0635 x 2) = 19685.04.
    assert_eq!(want, 19_685.0, "chip per rev {}", chip_per_rev(&chart));
}

/// (c) A small X/Y rate binds the cutting ceiling; the op feed cap binds
/// through the same ceiling (ruling Q11).
#[test]
fn a_small_xy_rate_binds_the_cutting_ceiling_p21() {
    let (op, tool) = (pocket(), flat_2f(6.0));
    let chart = run(&op, &tool, &wanaka_like(), SpindleStrategy::MatchChart);
    // An X/Y rate between the chart feed and the feed at the spindle
    // maximum: the mean of the two.
    let (_, spindle_max) = wanaka_like().rpm_range();
    let xy = 0.5 * (chart.feed_rate_mm_min + chip_per_rev(&chart) * spindle_max);
    let machine = with_rates(xy);
    let r = run(&op, &tool, &machine, constant_chip(Some(CEILING)));
    let c = choice(&r);
    assert_eq!(
        c.binding,
        RpmLimit::CuttingCeiling(CuttingCeilingSource::AxisRate),
        "{:?}",
        c.limits
    );
    let want = (xy / chip_per_rev(&chart)).floor();
    assert_eq!(c.rpm, want);
    assert_eq!(
        r.rpm, want,
        "Step 7 does not act: the feed is under the ceiling"
    );
    assert!(
        chart.rpm < want && want < spindle_max,
        "non-vacuity: {want}"
    );
    assert!(r.feed_rate_mm_min <= xy);

    // Q11: the op feed cap lowers the one ceiling of Step 5c and Step 7.
    let material = white_oak();
    let cap = xy;
    let wanaka = wanaka_like();
    let input = FeedsInput::of_operation(
        &op,
        &tool,
        &material,
        &wanaka,
        embedded_vendor_lut(),
        constant_chip(Some(CEILING)),
    )
    .with_feed_max_rate(cap);
    let capped = calculate(&input);
    let c = choice(&capped);
    assert_eq!(
        c.binding,
        RpmLimit::CuttingCeiling(CuttingCeilingSource::OpFeedCap),
        "{:?}",
        c.limits
    );
    assert_eq!(capped.rpm, want);
    // Under `MatchChart`, a cap below the chart feed binds Step 7.
    let low_cap = 0.5 * chart.feed_rate_mm_min;
    let input = FeedsInput::of_operation(
        &op,
        &tool,
        &material,
        &wanaka,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    )
    .with_feed_max_rate(low_cap);
    let lowered = calculate(&input);
    assert!(
        lowered.feed_rate_mm_min <= low_cap,
        "Step 7 holds the op feed cap: {} vs {low_cap}",
        lowered.feed_rate_mm_min
    );
    assert_eq!(
        lowered.derates.spindle_scale_reason,
        SpindleScaleReason::FeedCeiling
    );
}

/// The required power per RPM of the chart run, kW per RPM: at a held chip
/// the required power is linear in the RPM (`P21_DESIGN.md` section 2.5).
fn power_per_rpm(chart: &FeedsResult) -> f64 {
    chart.power_kw.expect("the pocket power is modelled") / chart.rpm
}

/// (d) and (e): the power binds at `P / k` on a constant-power spindle, and
/// at `n_r / R` on a constant-torque drive above its rated RPM.
#[test]
fn the_power_binds_at_the_rpm_where_the_cut_meets_the_curve_p21() {
    let (op, tool) = (pocket(), flat_2f(6.0));
    let chart = run(&op, &tool, &wanaka_like(), SpindleStrategy::MatchChart);
    assert!(!chart.power_limited, "the chart cut is inside the power");
    let k = power_per_rpm(&chart);
    let (_, spindle_max) = wanaka_like().rpm_range();
    // A power that the cut meets halfway between the chart RPM and the
    // spindle maximum, plus half an RPM: the whole RPM below it then fits
    // the power with a margin, not by a rounding step.
    let n_meet = 0.5 * (chart.rpm + spindle_max) + 0.5;
    let power_kw = k * n_meet;

    // (d) ConstantPower: n_power = P / k.
    let machine = MachineProfile {
        power: PowerModel::ConstantPower { power_kw },
        ..wanaka_like()
    };
    let r = run(&op, &tool, &machine, constant_chip(Some(CEILING)));
    let c = choice(&r);
    assert_eq!(c.binding, RpmLimit::PowerAtRpm, "{:?}", c.limits);
    let n_power = limit_rpm(&r, RpmLimit::PowerAtRpm);
    assert!(
        (n_power - power_kw / k).abs() < 1e-6 * n_meet,
        "n_power {n_power} vs P / k {}",
        power_kw / k
    );
    assert_eq!(c.rpm, n_power.floor());
    assert_eq!(r.rpm, c.rpm, "the cut fits: the ladder does not act");
    assert!(!r.power_limited);

    // (e) A constant-torque drive with a rated RPM below the chart RPM:
    // above n_r the available power is P_r, so n_power = n_r / R with
    // R = k x n_r / P_r.
    let rated_rpm = 0.5 * chart.rpm;
    let machine = MachineProfile {
        power: PowerModel::VfdConstantTorque {
            rated_power_kw: power_kw,
            rated_rpm,
        },
        ..wanaka_like()
    };
    let r = run(&op, &tool, &machine, constant_chip(Some(CEILING)));
    let c = choice(&r);
    assert_eq!(c.binding, RpmLimit::PowerAtRpm, "{:?}", c.limits);
    let ratio = k * rated_rpm / power_kw;
    let n_power = limit_rpm(&r, RpmLimit::PowerAtRpm);
    assert!(
        (n_power - rated_rpm / ratio).abs() < 1e-6 * n_meet,
        "n_power {n_power} vs n_r / R {}",
        rated_rpm / ratio
    );
    assert_eq!(r.rpm, n_power.floor());
}

/// (f) and (g): the operator ceiling binds; an empty ceiling ships the
/// chart values.
#[test]
fn the_operator_ceiling_binds_and_an_empty_ceiling_ships_the_chart_p21() {
    let (op, tool, machine) = (pocket(), flat_2f(6.0), wanaka_like());
    let chart = run(&op, &tool, &machine, SpindleStrategy::MatchChart);
    let (_, spindle_max) = machine.rpm_range();
    // A ceiling halfway between the chart RPM and the spindle maximum.
    let ceiling = (0.5 * (chart.rpm + spindle_max)).round();
    let r = run(&op, &tool, &machine, constant_chip(Some(ceiling as u32)));
    let c = choice(&r);
    assert_eq!(c.binding, RpmLimit::OperatorCeiling, "{:?}", c.limits);
    assert_eq!(r.rpm, ceiling);
    assert!(
        (chip_per_rev(&r) - chip_per_rev(&chart)).abs() < 1e-9,
        "the chip holds"
    );

    // A ceiling below the chart RPM always binds (lead ruling
    // 2026-11-10): the RPM comes down to the ceiling, and the feed follows
    // to hold the chip.
    let low = (0.5 * (chart.rpm + machine.rpm_range().0)).round();
    assert!(
        low < chart.rpm,
        "non-vacuity: the ceiling {low} is below the chart RPM"
    );
    let r = run(&op, &tool, &machine, constant_chip(Some(low as u32)));
    assert_eq!(choice(&r).binding, RpmLimit::OperatorCeiling);
    assert_eq!(choice(&r).rpm, low);
    assert_eq!(r.rpm, low);
    assert!(
        (chip_per_rev(&r) - chip_per_rev(&chart)).abs() < 1e-9,
        "the chip holds under a low ceiling"
    );
    assert!(r.feed_rate_mm_min < chart.feed_rate_mm_min);

    // An empty ceiling: the chart RPM is the limit.
    let r = run(&op, &tool, &machine, constant_chip(None));
    assert_eq!(choice(&r).binding, RpmLimit::ChartRpm);
    assert_eq!(r.rpm, chart.rpm);
    assert_eq!(r.feed_rate_mm_min, chart.feed_rate_mm_min);
    assert_eq!(r.plunge_rate_mm_min, chart.plunge_rate_mm_min);
}

/// (h) A row that prints no RPM takes the lowest limit; the surface-speed
/// RPM of `MatchChart` is not read.
#[test]
fn a_row_with_no_printed_rpm_takes_the_lowest_limit_p21() {
    let (tool, machine) = (flat_2f(6.0), wanaka_like());
    // The first op of the list whose matched row prints no RPM.
    let found = [
        OperationType::Profile,
        OperationType::Pocket,
        OperationType::Adaptive,
        OperationType::Trace,
    ]
    .into_iter()
    .map(fixture_op)
    .find_map(|op| {
        let chart = run(&op, &tool, &machine, SpindleStrategy::MatchChart);
        let row = chart.matched_lut_row.as_ref()?;
        row.rpm_nominal.is_none().then_some((op, chart))
    });
    let (op, chart) = found.expect("non-vacuity: a 6 mm white oak row prints no RPM");
    let (_, spindle_max) = machine.rpm_range();
    assert!(
        chart.rpm < spindle_max,
        "non-vacuity: the surface-speed RPM {} is below the spindle maximum",
        chart.rpm
    );

    let r = run(&op, &tool, &machine, constant_chip(Some(CEILING)));
    let c = choice(&r);
    assert_eq!(c.chart_rpm, None, "the row prints no RPM");
    let lowest = c.limits.iter().map(|l| l.rpm).fold(f64::INFINITY, f64::min);
    assert_eq!(c.rpm, machine.next_rpm_at_or_below(lowest.floor()));
    assert_eq!(r.rpm, c.rpm);
    // On this machine the lowest limit is the spindle maximum
    // (`P21_DESIGN.md` section 3: n_xy = 15000 / (0.14919 x 2) = 50271).
    assert_eq!(r.rpm, spindle_max);
    assert!(
        (chip_per_rev(&r) - chip_per_rev(&chart)).abs() < 1e-9,
        "the chip is the chip of the row: {} vs {}",
        chip_per_rev(&r),
        chip_per_rev(&chart)
    );
}

/// The 18 mm tool of the wanaka project (tool 6: 2 flutes, 20 mm flute,
/// 6 mm shank, 25 mm stick-out).
fn wanaka_18mm() -> ToolConfig {
    let mut t = flat_2f(18.0);
    t.set_cutting_length(ToolFact::Stated(20.0));
    t.set_stickout(ToolFact::Stated(25.0));
    t
}

/// (i) The probe of the phase note: the 18 mm tool in white oak on the
/// wanaka-like machine, ceiling 24000, through the production Suggest door
/// (`suggest_for_operation`, which calls `FeedsInput::of_operation`). The
/// first row is the wanaka idx 1 cut: Adaptive 3D, stepover 5.5, depth per
/// pass 10. `--nocapture` prints the values.
#[test]
#[expect(clippy::print_stderr, reason = "the probe prints its values")]
fn the_18_mm_probe_names_its_binding_limit_p21() {
    let (tool, machine) = (wanaka_18mm(), wanaka_like());
    let mut wanaka_idx1 = fixture_op(OperationType::Adaptive3d);
    assert!(wanaka_idx1.set_stepover(5.5) && wanaka_idx1.set_depth_per_pass(10.0));
    let rows = [
        ("Adaptive3d ae 5.5 ap 10 (wanaka idx 1)", wanaka_idx1),
        ("Pocket (fixture values)", fixture_op(OperationType::Pocket)),
        (
            "Adaptive 2D (fixture values)",
            fixture_op(OperationType::Adaptive),
        ),
    ];
    for (label, op) in rows {
        for strategy in [SpindleStrategy::MatchChart, constant_chip(Some(CEILING))] {
            let s = suggest_for_operation(SuggestForOperationInput {
                operation: &op,
                tool: &tool,
                machine: &machine,
                material: &white_oak(),
                lut: embedded_vendor_lut(),
                spindle_strategy: strategy,
                context: SuggestContext::default(),
            })
            .unwrap_or_else(|e| panic!("Suggest serves {label}: {e}"));
            let r = &s.feeds_result;
            eprintln!(
                "P21c probe | {label} | {strategy:?} | calc rpm {} feed {:.1} feed/tooth {:.5} | \
                 target chip {:.5} radial lift {:.4} | row {:?} prints rpm {:?} band {:?} | \
                 ae {:.3} ap {:.3} | power {:?} of {:.3} kW limited {} | shipped feed {:?} rpm {:?} \
                 stepover {:?} dpp {:?} | choice {:?} | reason {:?} | warnings {:?}",
                r.rpm,
                r.feed_rate_mm_min,
                r.feed_rate_mm_min / (r.rpm * 2.0),
                r.chip_load_mm,
                r.derates.radial_chip_thinning,
                r.vendor_source,
                r.matched_lut_row.as_ref().map(|row| row.rpm_nominal),
                r.chipload_bounds
                    .map(|b| (b.min_mm_per_tooth, b.max_mm_per_tooth)),
                r.radial_width_mm,
                r.axial_depth_mm,
                r.power_kw,
                r.available_power_kw,
                r.power_limited,
                s.operation.feed_rate(),
                s.operation.spindle_rpm(),
                s.operation.stepover(),
                s.operation.depth_per_pass(),
                r.rpm_choice,
                r.derates.spindle_scale_reason,
                r.warnings,
            );
            if let Some(c) = r.rpm_choice.as_ref() {
                assert!(r.rpm <= c.rpm, "a later step only lowers the RPM");
            }
        }
    }
}
