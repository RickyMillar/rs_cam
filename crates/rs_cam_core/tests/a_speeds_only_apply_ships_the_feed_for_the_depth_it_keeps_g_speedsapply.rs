//! **G-SPEEDSAPPLY — a speeds-only apply ships the feed for the depth it
//! keeps.**
//!
//! Found on 2026-10-12 by
//! `a_feed_follows_the_depth_through_the_calculator_g_opt5`, red on the
//! Suggest funnel route before the optimizer stopped using that route. A
//! 3 mm 2-flute flat end mill in oak, Pocket / roughing, at a depth per pass
//! of 7.5 mm (2.5 x D, deeper than the 0.25 x D rigidity rule; P13c: deeper
//! than the force-budget depth). The
//! calculator at 7.5 mm gives 2423 mm/min. `apply_speeds_to_op` shipped
//! 3877.8 mm/min: the funnel wrote the result onto a scratch copy, the
//! rigidity clamp moved the scratch depth to 0.75 mm, pass 9 re-derived the
//! feed for 0.75 mm, and the funnel copied that feed onto an operation that
//! still cuts 7.5 mm.
//!
//! The rule (`feeds::suggest::apply`, doc of `apply_feeds_subset`): under
//! `ApplyScope::Speeds` the passes that move the geometry do not run, and
//! the feed-side passes read the geometry the operation keeps. Pass 9
//! re-derives the feed from the calculator's geometry to the kept geometry.
//!
//! The arms:
//!
//! 1. A result sized at the kept depth (7.5 mm) ships its own feed, rounded
//!    down to 1 mm/min, and the depth and the stepover stay. Non-vacuity, in
//!    two materials (feeds contract P18, lead ruling):
//!    - the oak of the fixture is a Janka-only species with no force line.
//!      P16 item 6c gives no depth proposal without a force line, so "Apply
//!      all" (the envelope proposal; before feeds contract P5 the combined
//!      chain apply `Both`) keeps 7.5 mm. The envelope still states its
//!      depth ceiling below 7.5 mm and places the kept depth outside it, so
//!      the operator sees the warning with no proposal;
//!    - generic hardwood has a force line, so "Apply all" writes a depth
//!      proposal: the depth lowered to its binding cap.
//!
//!    P16 (D12) deleted the slot step, so no slot limit sets a ceiling.
//! 2. The Suggest door's own result (the calculator picks the depth, as the
//!    GUI "Apply recommended speeds" button does) ships a feed below the
//!    feed of "Apply all": the depth de-rate for 7.5 mm reached the feed.
//! 3. Control: on an operation that already carries the geometry "Apply
//!    all" writes (inside the rigidity cap), the speeds are bit-identical
//!    to the speeds of "Apply all". P18: in generic hardwood, where "Apply
//!    all" writes a depth (the oak gets no depth proposal, P16 item 6c).
//!
//! The tool and the stock are the fixture of the G-OPT5 sentry (the mini
//! project, `planning/rivmap_block_job/testpiece_mini.toml`, tool 0 and the
//! `SolidWoodByJanka` stock). This file builds its own copy and does not
//! call the optimizer.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::envelope::PickRequest;
use rs_cam_core::feeds::envelope::Placement;
use rs_cam_core::feeds::suggest::{
    NoProject, OperationApplyInput, SuggestContext, apply_speeds_to_op, apply_to_operation,
    feeds_result_for_operation, round_suggestion_value_down,
};
use rs_cam_core::feeds::{
    FeedsProvenance, FeedsResult, SpindleStrategy, calculate, embedded_vendor_lut,
    validate_tool_for_operation,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The depth per pass of the defect: 2.5 x the 3 mm diameter.
const DEEP_DPP_MM: f64 = 7.5;
/// The stepover of the mini project's pocket.
const STEPOVER_MM: f64 = 0.838;

/// Tool 0 of the mini project: 3 mm, 2 flutes, 30 mm cut length and
/// stick-out.
fn mini_tool() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_cutting_length(ToolFact::Stated(30.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_shank_diameter(ToolFact::Stated(3.0));
    tool
}

/// The mini project's stock material.
fn mini_oak() -> Material {
    Material::SolidWoodByJanka {
        janka_lbf: 1290.0,
        label: "Oak (feeds_oak.toml janka_lbf)".to_owned(),
        source_id: "rivmap feeds_oak.toml".to_owned(),
    }
}

/// A 30 mm pocket at the given depth per pass. 30 / 7.5 = 4 passes, so the
/// funnel's depth staircase keeps 7.5 mm.
fn pocket(depth_per_pass: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: Some(STEPOVER_MM),
        floor: rs_cam_core::compute::floor::Floor::top(30.0),
        depth_per_pass: Some(depth_per_pass),
        feed_rate: Some(3877.0),
        plunge_rate: Some(1938.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

struct Fixture {
    tool: ToolConfig,
    material: Material,
    machine: MachineProfile,
}

impl Fixture {
    fn new() -> Self {
        Self::with(mini_oak())
    }

    /// The fixture in `material` (P18: a material with a force line gets a
    /// depth proposal; the oak does not).
    fn with(material: Material) -> Self {
        Self {
            tool: mini_tool(),
            material,
            machine: MachineProfile::generic_wood_router(),
        }
    }

    /// Generic hardwood: a species with a force line (B6).
    fn hardwood() -> Self {
        Self::with(Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        })
    }

    /// The Suggest door's result: the calculator picks the depth (a Pocket
    /// gives it no depth hint).
    fn door_result(&self, op: &OperationConfig) -> FeedsResult {
        feeds_result_for_operation(
            op,
            &self.tool,
            &self.material,
            &self.machine,
            embedded_vendor_lut(),
            SpindleStrategy::default(),
        )
        .expect("a flat end mill on a Pocket is a valid pairing")
    }

    /// The calculator run at the operation's own depth and stepover.
    fn result_at(&self, op: &OperationConfig, dpp: f64, stepover: f64) -> FeedsResult {
        let mut op = op.clone();
        assert!(op.set_depth_per_pass(dpp));
        assert!(op.set_stepover(stepover));
        let input = FeedsInput::of_operation(
            &op,
            &self.tool,
            &self.material,
            &self.machine,
            embedded_vendor_lut(),
            SpindleStrategy::default(),
        );
        validate_tool_for_operation(&input).expect("a flat end mill on a Pocket is valid");
        calculate(&input)
    }

    fn speeds(&self, op: &OperationConfig, result: &FeedsResult) -> OperationConfig {
        let mut applied = op.clone();
        let mut prov = FeedsProvenance::default();
        apply_speeds_to_op(
            &mut applied,
            &mut prov,
            result,
            &self.tool,
            &self.machine,
            &self.material,
            op.feeds_style().1,
            SuggestContext::no_project(NoProject::NONE),
        );
        applied
    }

    /// "Apply all": `Command::ApplyFeeds` with the envelope proposal
    /// (feeds contract P5, K2b). The proposal lowers the stated depth to its
    /// binding cap and the speeds are sized at the pick. It replaced the
    /// combined chain apply (`Both`) that this file compared with before P5.
    fn apply_all(&self, op: &OperationConfig) -> OperationConfig {
        let mut applied = op.clone();
        let mut prov = FeedsProvenance::default();
        apply_to_operation(
            OperationApplyInput {
                tool: &self.tool,
                machine: &self.machine,
                material: &self.material,
                spindle_strategy: SpindleStrategy::default(),
                lut: embedded_vendor_lut(),
                suggest: SuggestContext::no_project(NoProject::NONE),
            },
            Some(PickRequest::Proposal),
            None,
            &mut applied,
            &mut prov,
        )
        .expect("a flat end mill on a Pocket is a valid pairing");
        applied
    }

    /// The lowest depth ceiling (mm) of the envelope of `op` at its own
    /// width. P13c (O1): the 0.25 x D rigidity rule (0.75 mm on this tool)
    /// is gone. In the oak (a Janka-only species, no FPL density) there is no
    /// force line, so the force-budget limit abstains and another limit (the
    /// vendor depth table) sets the ceiling; P16 (D12) deleted the slot step.
    /// In generic hardwood the lowest ceiling at this width is the same
    /// vendor depth table (measured 2026-11-07).
    fn rigidity_ceiling(&self, op: &OperationConfig) -> f64 {
        let ae = op.stepover().expect("a pocket carries a stepover");
        let c = self
            .envelope(op)
            .ap_ceiling_at(ae)
            .expect("a roughing pocket carries a depth ceiling");
        c.0
    }

    /// The safe envelope of `op`.
    fn envelope(&self, op: &OperationConfig) -> rs_cam_core::feeds::envelope::SafeEnvelope {
        rs_cam_core::feeds::envelope::SafeEnvelope::for_cut(
            rs_cam_core::feeds::envelope::EnvelopeInput {
                operation: op,
                tool: &self.tool,
                machine: &self.machine,
                material: &self.material,
                lut: embedded_vendor_lut(),
                spindle_strategy: SpindleStrategy::default(),
                context: SuggestContext::no_project(NoProject::NONE),
            },
        )
        .expect("a pocket envelope")
    }
}

/// Arm 1: a result sized at 7.5 mm ships its own feed on an operation that
/// keeps 7.5 mm.
#[test]
fn a_speeds_only_apply_ships_the_calculator_feed_for_the_kept_depth_g_speedsapply() {
    let f = Fixture::new();
    let op = pocket(DEEP_DPP_MM);
    let result = f.result_at(&op, DEEP_DPP_MM, STEPOVER_MM);
    assert!(
        (result.axial_depth_mm - DEEP_DPP_MM).abs() < 1e-9,
        "fixture drift: the calculator did not honour the pinned depth: {}",
        result.axial_depth_mm
    );
    // Pass 9 must see no move for the feed to stay the result's feed, so
    // the calculator must also keep the pinned stepover.
    assert!(
        (result.radial_width_mm - STEPOVER_MM).abs() < 1e-9,
        "fixture drift: the calculator did not honour the pinned stepover: {}",
        result.radial_width_mm
    );
    let applied = f.speeds(&op, &result);

    assert_eq!(
        applied.depth_per_pass(),
        Some(DEEP_DPP_MM),
        "a speeds-only apply moved the depth per pass"
    );
    assert_eq!(
        applied.stepover(),
        Some(STEPOVER_MM),
        "a speeds-only apply moved the stepover"
    );
    let expected_feed = round_suggestion_value_down(result.feed_rate_mm_min, 1.0);
    assert_eq!(
        applied.feed_rate().map(f64::to_bits),
        Some(expected_feed.to_bits()),
        "the operation keeps {DEEP_DPP_MM} mm, and the calculator at that depth \
         gives {expected_feed} mm/min; the speeds-only apply ships {} mm/min",
        applied.feed_rate().expect("S11: the fixture sets it")
    );
    assert_eq!(
        applied.spindle_rpm(),
        Some(result.rpm.round() as u32),
        "the RPM is the calculator's"
    );

    // Non-vacuity in the oak, a pin of P16 item 6c: no force line, so no
    // depth proposal. "Apply all" keeps the stated depth; the envelope still
    // states its depth ceiling below it and places the kept depth outside
    // it (the warning with no proposal).
    let all = f.apply_all(&op);
    assert_eq!(
        all.depth_per_pass(),
        Some(DEEP_DPP_MM),
        "P16 item 6c: no force line, no depth proposal, so Apply all keeps the depth"
    );
    let ceiling = f.rigidity_ceiling(&all);
    assert!(
        ceiling < DEEP_DPP_MM,
        "the envelope states a depth ceiling ({ceiling} mm) below the kept {DEEP_DPP_MM} mm"
    );
    match f.envelope(&all).operator {
        Some(Placement::Outside { binding, margin }) => {
            assert!(margin < 0.0, "outside by {margin} at {binding:?}");
        }
        other => panic!("the kept {DEEP_DPP_MM} mm sits outside the envelope: {other:?}"),
    }

    // Non-vacuity in generic hardwood (a force line, so P16 item 6c gives a
    // depth proposal): "Apply all" writes `min(stated, binding cap)`
    // (`lower_to_cap`) at the stated width. Measured 2026-11-07: the binding
    // cap is the envelope ceiling at 0.838 mm, 3.0028 mm (the vendor depth
    // table, as in the oak), and the depth staircase of the 30 mm pocket
    // snaps it to 30 / ceil(30 / 3.0028) = 3.0 mm. The test reads the
    // ceiling from the envelope and types no number. Both feeds sit on the
    // generic router's cutting cap, 3999 mm/min after the round-down (T-9).
    let hw = Fixture::hardwood();
    let hw_result = hw.result_at(&op, DEEP_DPP_MM, STEPOVER_MM);
    let hw_applied = hw.speeds(&op, &hw_result);
    let hw_all = hw.apply_all(&op);
    let all_dpp = hw_all.depth_per_pass().expect("a pocket carries a DPP");
    assert!(
        all_dpp < DEEP_DPP_MM && all_dpp <= hw.rigidity_ceiling(&hw_all) + 1e-9,
        "Apply all lowers the depth in hardwood ({all_dpp} mm, ceiling {} mm)",
        hw.rigidity_ceiling(&hw_all)
    );
    // Feeds contract P11b (L6): the depth no longer scales the feed (the
    // vendor depth table is an envelope limit), so the feed of the shallower
    // depth is not below the feed of the kept depth.
    assert!(
        hw_all.feed_rate() >= hw_applied.feed_rate(),
        "the feed of Apply all ({} mm/min, at {all_dpp} mm) must not be below the feed \
         for {DEEP_DPP_MM} mm ({} mm/min)",
        hw_all.feed_rate().expect("S11: the apply writes it"),
        hw_applied.feed_rate().expect("S11: the fixture sets it")
    );
}

/// Arm 2: the Suggest door's result, as the GUI button applies it. The
/// calculator picks its own depth; the funnel re-derives the feed to the
/// kept 7.5 mm, and that feed is below the feed of `Both`.
#[test]
fn the_door_result_is_derated_to_the_kept_depth_g_speedsapply() {
    let f = Fixture::new();
    let op = pocket(DEEP_DPP_MM);
    let result = f.door_result(&op);
    // Feeds contract P5 (K2): the calculator reads the op's stated depth,
    // not a family default, so the door result is sized at the kept depth
    // (or under it, where a calculator cap such as the flute guard binds).
    assert!(
        result.axial_depth_mm <= DEEP_DPP_MM,
        "the calculator picked {} mm, above the kept depth",
        result.axial_depth_mm
    );

    let applied = f.speeds(&op, &result);
    let all = f.apply_all(&op);

    assert_eq!(applied.depth_per_pass(), Some(DEEP_DPP_MM));
    assert_eq!(applied.stepover(), Some(STEPOVER_MM));
    // P11b (L6): the depth no longer scales the feed, so the kept depth's
    // feed is at most the feed of Apply all (equal on the feed cap here).
    assert!(
        applied.feed_rate() <= all.feed_rate(),
        "the speeds-only feed ({} mm/min, for {DEEP_DPP_MM} mm) must not be above the \
         feed of Apply all ({} mm/min, for {:?} mm)",
        applied.feed_rate().expect("S11: the fixture sets it"),
        all.feed_rate().expect("S11: the apply writes it"),
        all.depth_per_pass()
    );
}

/// Arm 3, the control: an operation that already carries the geometry
/// "Apply all" writes (inside the rigidity cap). The speeds-only apply on
/// it ships the speeds of "Apply all" bit for bit. Feeds contract P5:
/// "Apply all" (the envelope proposal) replaced the combined chain apply.
#[test]
fn inside_the_cap_the_speeds_equal_the_combined_apply_g_speedsapply() {
    // P18: generic hardwood, where Apply all writes a depth (P16 item 6c:
    // the oak has no force line and gets no depth proposal).
    let f = Fixture::hardwood();
    let deep = pocket(DEEP_DPP_MM);
    let at_proposal = f.apply_all(&deep);
    let dpp = at_proposal
        .depth_per_pass()
        .expect("a pocket carries a DPP");
    assert!(
        dpp <= f.rigidity_ceiling(&at_proposal) + 1e-9,
        "the control must sit inside the rigidity cap: {dpp} mm"
    );

    let result = f.door_result(&at_proposal);
    let speeds = f.speeds(&at_proposal, &result);
    let again = f.apply_all(&at_proposal);

    assert_eq!(
        speeds.feed_rate().map(f64::to_bits),
        again.feed_rate().map(f64::to_bits),
        "feed: speeds-only {} vs Apply all {}",
        speeds.feed_rate().expect("S11: the fixture sets it"),
        again.feed_rate().expect("S11: the apply writes it")
    );
    assert_eq!(
        speeds.plunge_rate().map(f64::to_bits),
        again.plunge_rate().map(f64::to_bits),
        "plunge: speeds-only {} vs Apply all {}",
        speeds.plunge_rate().expect("S11: the fixture sets it"),
        again.plunge_rate().expect("S11: the apply writes it")
    );
    assert_eq!(speeds.spindle_rpm(), again.spindle_rpm(), "rpm");
    assert_eq!(speeds.depth_per_pass(), at_proposal.depth_per_pass());
    assert_eq!(speeds.stepover(), at_proposal.stepover());
    // Apply all on its own output writes the same geometry again.
    assert_eq!(again.depth_per_pass(), at_proposal.depth_per_pass());
    assert_eq!(again.stepover(), at_proposal.stepover());
}
