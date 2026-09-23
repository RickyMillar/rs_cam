//! **A feed lift caps at the CUTTING ceiling, not at the gantry travel rate**
//! (T-18, `planning/TECH_DEBT_REGISTER.md`).
//!
//! ## The two quantities
//!
//! A [`MachineProfile`] carries two feed limits and they are not
//! interchangeable:
//!
//! - `max_feed_mm_min` — the gantry TRAVEL rate. What the axes move at.
//! - `cutting_feed_ceiling_mm_min()` — what the machine may CUT at. It never
//!   exceeds the travel rate.
//!
//! ## One axis since ruling R4 (2026-09-24)
//!
//! Until R4, Step 9 multiplied the feed by `safety_factor`, so the calculator
//! had a RAW axis (before Step 9) and a COMMANDED axis (after it), and the
//! commanded ceiling was `cutting_feed_ceiling_mm_min() * safety_factor`.
//! Ruling R4 removed the factor. The two axes are one, and the calculator's
//! output invariant is:
//!
//! ```text
//! commanded_feed <= cutting_feed_ceiling_mm_min()
//! ```
//!
//! ## The defect
//!
//! Three lifts ran AFTER Step 9, on the COMMANDED axis, and each one capped
//! at `max_feed_mm_min * safety_factor` — a fraction of the TRAVEL rate:
//!
//! - `feeds/mod.rs` Step 9b, the rubbing-floor lift.
//! - `feeds/mod.rs` Step 9c, the drill envelope clamp.
//! - `feeds/suggest/adaptive_entry.rs` pass 9, the re-applied floor lift.
//!
//! Each lift therefore restored a feed above the ceiling Step 7 enforced.
//!
//! ## Ruling R4 WP2a (2026-09-23): two of the three lifts are gone
//!
//! Step 9b and pass 9 now warn and do not lift a feed, so they cannot
//! restore a feed above any ceiling. Their arms are retired; the sentry
//! `rubbing_floor_warns_and_never_lifts` pins the new behaviour. The Step 9c
//! drill envelope still clamps, and its arm stays.
//!
//! ## The fixture
//!
//! [`fast_gantry_slow_cut`]: travel 10 000 mm/min and an explicit cutting
//! ceiling of 500 mm/min. The drill plunge-feed envelope floor on a 12 mm
//! cutter in solid wood is 600 mm/min, between the two. The defect lands the
//! feed at 600, the fix at 500. (Before ruling R4 the fixture carried
//! `safety_factor` 0.8, and the two caps were 400 and 8 000.)
//!
//! ## Non-vacuity
//!
//! `the_fix_is_silent_on_every_shipped_preset` asserts the two expressions
//! are numerically EQUAL on all three presets, because every preset's travel
//! rate sits under `DEFAULT_CUTTING_FEED_CAP_MM_MIN`. The swap therefore
//! moves no shipped number. The same test runs the fixture operation on a
//! preset and shows its feed lands above the slow-ceiling cap, so the fixture
//! machine discriminates and the arm above is not an accident of it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, WorkholdingRigidity, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// Relative tolerance for a feed comparison in mm/min.
const EPS: f64 = 1e-6;

// ── Fixtures ────────────────────────────────────────────────────────────

/// A fast gantry with a slow cutting ceiling. The profile the register's
/// T-18 entry describes: the two limits differ, so the two candidate caps
/// differ by a factor of 20.
fn fast_gantry_slow_cut() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.max_feed_mm_min = 10_000.0;
    machine.max_cutting_feed_mm_min = Some(500.0);
    machine
}

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// A 6 mm two-flute flat end mill.
fn flat_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shaft_diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.stickout = 30.0;
    tool.flute_count = 2;
    tool
}

/// A 12 mm two-flute drill. The diameter puts the material's plunge-feed
/// envelope floor above the commanded cutting ceiling of the fixture, which
/// is what makes the Step 9c cap observable.
fn drill_12mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(2), ToolType::EndMill);
    tool.diameter = 12.0;
    tool.cutting_length = 60.0;
    tool.shaft_diameter = 12.0;
    tool.shank_diameter = 12.0;
    tool.stickout = 70.0;
    tool.flute_count = 2;
    tool
}

/// The calculator over a pocket rough. `spindle_strategy` is the shipped
/// default so the rpm is the engine's own choice.
fn pocket_rough(machine: &MachineProfile, tool: &ToolConfig) -> FeedsResult {
    let material = white_oak();
    calculate(&FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(3.0),
        radial_width_mm: Some(2.0),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext {
            tool_overhang_mm: Some(tool.stickout),
            workholding_rigidity: WorkholdingRigidity::Medium,
        },
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// The calculator over a drill cycle.
fn drill_cycle(machine: &MachineProfile, tool: &ToolConfig) -> FeedsResult {
    let material = white_oak();
    calculate(&FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(12.0),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext {
            tool_overhang_mm: Some(tool.stickout),
            workholding_rigidity: WorkholdingRigidity::Medium,
        },
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// The cap the travel rate would give. The test states it in full so the
/// failure message names both candidates.
fn travel_rate_cap(machine: &MachineProfile) -> f64 {
    machine.max_feed_mm_min
}

// ── 2. Step 9c — the drill envelope clamp ───────────────────────────────

#[test]
fn the_drill_envelope_clamp_caps_at_the_cutting_ceiling() {
    let machine = fast_gantry_slow_cut();
    let tool = drill_12mm();
    let result = drill_cycle(&machine, &tool);

    let clamped = result
        .warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::DrillFeedClampedToEnvelope { .. }));
    assert!(
        clamped,
        "non-vacuity: the fixture must actually reach the drill envelope clamp; at rpm \
         {:.0} it shipped {:.3} mm/min and its warnings were {:?}",
        result.rpm, result.feed_rate_mm_min, result.warnings
    );

    let cap = machine.cutting_feed_ceiling_mm_min();
    assert!(
        result.feed_rate_mm_min <= cap * (1.0 + EPS),
        "the drill envelope clamp restored {:.3} mm/min at rpm {:.0}, above the \
         cutting ceiling {cap:.3} mm/min; it capped against the travel rate {:.3} mm/min \
         instead",
        result.feed_rate_mm_min,
        result.rpm,
        travel_rate_cap(&machine),
    );
}

// ── 4. Non-vacuity — the swap is silent on every shipped preset ─────────

#[test]
fn the_fix_is_silent_on_every_shipped_preset() {
    let presets = [
        MachineProfile::generic_wood_router(),
        MachineProfile::shapeoko_vfd(),
        MachineProfile::shapeoko_makita(),
    ];
    for preset in &presets {
        assert!(
            (preset.cutting_feed_ceiling_mm_min() - preset.max_feed_mm_min).abs() < 1e-9,
            "{}: the preset's travel rate sits under the default cutting cap, so the \
             two quantities must be equal; ceiling {:.3}, travel {:.3}",
            preset.name,
            preset.cutting_feed_ceiling_mm_min(),
            preset.max_feed_mm_min,
        );
        let cutting_cap = preset.cutting_feed_ceiling_mm_min();
        assert!(
            (cutting_cap - travel_rate_cap(preset)).abs() < 1e-9,
            "{}: the cap this change installs must equal the cap it replaces; \
             {cutting_cap:.3} against {:.3}",
            preset.name,
            travel_rate_cap(preset),
        );
    }

    // The fixture machine discriminates: an operation on a shipped
    // preset lands far above the slow-ceiling fixture's cap.
    let preset = MachineProfile::generic_wood_router();
    let result = pocket_rough(&preset, &flat_endmill_6mm());
    let fixture_cap = fast_gantry_slow_cut().cutting_feed_ceiling_mm_min();
    assert!(
        result.feed_rate_mm_min > fixture_cap,
        "the preset feed {:.3} must sit above the fixture cap {fixture_cap:.3}, or the \
         fixture arm proves nothing",
        result.feed_rate_mm_min,
    );
}
