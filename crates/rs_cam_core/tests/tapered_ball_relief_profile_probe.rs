//! Operator probe for the two-sided relief gate.
//!
//! Tool taken from the LIVE PROJECT, not from memory: `wanaka.toml` tool
//! id 2, `diameter: 1.0`, `taper_half_angle_deg: 7.0`, Ø6 shank — i.e.
//! **R0.5 (Ø1.0 tip), 7° per side**. Its display name says "2mm tip",
//! which is wrong; `diameter` on a tapered ball IS the tip diameter
//! (`TaperedBallEndmill::new(ball_diameter, ..)`). Same geometry as the
//! library's "R0.5mm x 6mm x 20mm 2F Tapered Ball" (id 7).
//!
//! Not a sentry — a REPORT. It asks the engine, through its own public
//! expressions, what it will actually do with this tool, so the numbers
//! that decide a first real cut are measured rather than assumed.
//!
//! Run with:
//!   cargo test -p rs_cam_core --test tapered_ball_relief_profile_probe -- --nocapture
//!
//! Three things it exists to surface, all flagged in CLAUDE.md as soft:
//!
//! 1. A tapered ball's engaged diameter is a FUNCTION OF DOC, not a
//!    constant. Everything downstream (chipload band, feed, stepover)
//!    moves with it.
//! 2. Sub-Ø2 chipload verdicts are provisional — the diameter and
//!    hardness scaling laws (`D^0.61`, `Janka^-0.5`) are repo-derived
//!    with no primary source. This tool is sub-Ø2 for its first several
//!    millimetres of engagement, i.e. squarely inside that regime.
//! 3. `stock_to_leave` is a VERTICAL offset. What remains measured
//!    normal to a wall sloped at A is `stock_to_leave · cos A`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::feeds::geometry::scallop_stepover;
use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
    calculate, effective_diameter, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const TIP_DIAMETER_MM: f64 = 1.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;
const SHANK_DIAMETER_MM: f64 = 6.0;

fn hint() -> ToolGeometryHint {
    ToolGeometryHint::TaperedBall {
        tip_radius: TIP_DIAMETER_MM / 2.0,
        taper_angle_deg: TAPER_HALF_ANGLE_DEG,
    }
}

/// What the engine thinks the tool engages at a given axial DOC.
fn engaged(ap_mm: f64) -> f64 {
    effective_diameter(hint(), TIP_DIAMETER_MM, SHANK_DIAMETER_MM, ap_mm)
}

#[test]
fn report_engaged_diameter_against_depth() {
    eprintln!(
        "\n=== TAPERED BALL Ø{TIP_DIAMETER_MM} tip / {TAPER_HALF_ANGLE_DEG}° per side / \
         Ø{SHANK_DIAMETER_MM} shank ===\n\n\
         ENGAGED DIAMETER vs AXIAL DOC (the engine's own `effective_diameter`)\n\
         {:>10}  {:>12}  {:>10}\n\
         {:>10}  {:>12}  {:>10}",
        "DOC (mm)", "engaged Ø", "sub-Ø2?", "--------", "----------", "-------"
    );

    let mut first_over_2 = None;
    for ap in [0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0] {
        let d = engaged(ap);
        if d >= 2.0 && first_over_2.is_none() {
            first_over_2 = Some(ap);
        }
        eprintln!(
            "{ap:>10.2}  {d:>12.4}  {:>10}",
            if d < 2.0 { "PROVISIONAL" } else { "ok" }
        );
    }

    match first_over_2 {
        Some(ap) => eprintln!(
            "\n  -> engaged Ø reaches 2 mm at about {ap:.2} mm DOC. BELOW that the \
             chipload band rests on repo-derived scaling (D^0.61, Janka^-0.5) with \
             no primary source. A relief finish pass lives entirely in that regime."
        ),
        None => eprintln!(
            "\n  -> engaged Ø never reaches 2 mm across the sampled depths: every \
             cut with this tool is in the PROVISIONAL chipload regime."
        ),
    }

    // The measurement that matters: this is not a constant-diameter tool.
    let shallow = engaged(0.1);
    let deep = engaged(5.0);
    assert!(
        deep > shallow * 1.5,
        "probe is vacuous: engaged diameter barely moved with DOC \
         ({shallow:.4} -> {deep:.4}), so this tool is not behaving like a taper"
    );
}

#[test]
fn report_scallop_stepover_and_pass_count() {
    let tip_r = TIP_DIAMETER_MM / 2.0;
    eprintln!(
        "\nSCALLOP STEPOVER at the TIP (ball radius {tip_r} mm)\n\
         {:>16}  {:>12}  {:>16}\n\
         {:>16}  {:>12}  {:>16}",
        "scallop (mm)",
        "stepover",
        "passes / 100 mm",
        "------------",
        "----------",
        "---------------"
    );
    for scallop in [0.005, 0.01, 0.02, 0.05, 0.1] {
        match scallop_stepover(tip_r, scallop) {
            Some(so) => eprintln!("{scallop:>16.3}  {so:>12.4}  {:>16.0}", 100.0 / so),
            None => eprintln!("{scallop:>16.3}  {:>12}  {:>16}", "n/a", "-"),
        }
    }
    eprintln!(
        "\n  -> a Ø0.5 tip cannot take a big stepover: the tip radius caps it. \
         Expect a long finishing pass, and note the stepover the SCALLOP asks for \
         is unrelated to the engaged diameter above — one is set by the tip, the \
         other by depth."
    );

    // Non-vacuity: the helper must actually refuse a scallop it cannot reach.
    assert!(
        scallop_stepover(tip_r, tip_r * 2.0).is_none(),
        "scallop_stepover accepted a scallop deeper than the tip radius"
    );
}

#[test]
fn report_stock_to_leave_on_sloped_walls() {
    eprintln!(
        "\nSTOCK-TO-LEAVE IS VERTICAL — what actually remains normal to a wall\n\
         {:>12}  {:>10}  {:>12}  {:>12}",
        "wall angle", "cos A", "leave 0.2mm", "leave 0.5mm"
    );
    eprintln!(
        "{:>12}  {:>10}  {:>12}  {:>12}",
        "----------", "-----", "-----------", "-----------"
    );
    for deg in [0.0_f64, 15.0, 30.0, 45.0, 60.0, 75.0, 85.0] {
        let c = deg.to_radians().cos();
        eprintln!(
            "{deg:>12.0}  {c:>10.4}  {:>12.4}  {:>12.4}",
            0.2 * c,
            0.5 * c
        );
    }
    eprintln!(
        "\n  -> on a steep relief sidewall the finish allowance is a fraction of \
         what was dialled. At 75° a 0.2 mm leave is {:.3} mm — likely inside the \
         finish pass's own noise, i.e. effectively no allowance at all.",
        0.2 * 75.0_f64.to_radians().cos()
    );
}

/// The number that actually decides the cut: what Suggest commands for
/// this tool on the real machine and material, and where that lands
/// inside the vendor band it is judged against.
#[test]
fn report_what_suggest_commands_for_a_relief_finish() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };

    eprintln!(
        "\nWHAT SUGGEST COMMANDS — {} / White Oak\n\
         {:>9}  {:>10}  {:>8}  {:>11}  {:>13}  {:>11}",
        machine.name, "DOC (mm)", "engaged Ø", "rpm", "feed mm/min", "adv/tooth mm", "vs band"
    );
    eprintln!(
        "{:>9}  {:>10}  {:>8}  {:>11}  {:>13}  {:>11}",
        "--------", "---------", "-------", "----------", "------------", "----------"
    );

    let mut any_extrapolated = false;
    for ap in [0.1, 0.2, 0.3, 0.5, 1.0] {
        let d = engaged(ap);
        let input = FeedsInput {
            tool_diameter: TIP_DIAMETER_MM,
            flute_count: 2,
            flute_length: 25.0,
            shank_diameter: Some(SHANK_DIAMETER_MM),
            tool_geometry: hint(),
            material: &material,
            machine: &machine,
            operation: OperationFamily::Contour,
            operation_kind: None,
            pass_role: PassRole::Finish,
            axial_depth_mm: Some(ap),
            // A finish pass steps over by the scallop, not the diameter.
            radial_width_mm: Some(0.14),
            target_scallop_mm: Some(0.01),
            vendor_lut: Some(embedded_vendor_lut()),
            setup: SetupContext::default(),
            spindle_strategy: SpindleStrategy::MatchChart,
        };
        let r = calculate(&input);
        let advance = if r.rpm > 0.0 {
            r.feed_rate_mm_min / (r.rpm * 2.0)
        } else {
            0.0
        };
        let band = match r.chipload_bounds {
            Some(b) => {
                let pos = if advance < b.min_mm_per_tooth {
                    "UNDER"
                } else if advance > b.max_mm_per_tooth {
                    "OVER"
                } else {
                    "inside"
                };
                format!("{pos} {:.4}-{:.4}", b.min_mm_per_tooth, b.max_mm_per_tooth)
            }
            None => {
                any_extrapolated = true;
                "no band".to_owned()
            }
        };
        eprintln!(
            "{ap:>9.2}  {d:>10.4}  {:>8.0}  {:>11.1}  {advance:>13.5}  {band:>11}",
            r.rpm, r.feed_rate_mm_min
        );
        for w in &r.warnings {
            eprintln!("            ! {w:?}");
        }
    }

    if any_extrapolated {
        eprintln!(
            "\n  -> at least one depth had NO vendor band. With no band there is \
             nothing to judge the commanded feed against, so treat those rows as \
             unverified rather than approved."
        );
    }
    eprintln!(
        "\n  -> every row above engages well under Ø2, so the band itself is the \
         PROVISIONAL one (repo-derived D^0.61 / Janka^-0.5 scaling, no primary \
         source). Trust the SIM and the sound of the cut over these numbers, and \
         expect to adjust feed by ear on the first pass."
    );
}
