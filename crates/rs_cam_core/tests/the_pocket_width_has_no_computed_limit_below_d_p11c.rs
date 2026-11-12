//! Feeds contract P11c, D13 item 2, closed by P13c (rulings O3 and O4,
//! 2026-11-05).
//!
//! P11c held item 2 (the add-door stepover of a roughing Pocket from the
//! envelope) on a measured finding: at the add-door depth no computed
//! envelope limit bounded the width below the tool diameter (measured
//! 2026-11-05: the rigidity cap and the slot cap both equalled the add-door
//! depth, 0.25 D; power and the vendor depth table were above 1 D;
//! deflection read "no deflection limit below the flute"). The step 0
//! research confirmed it: no limit in the model falls with the width.
//!
//! P13c lands item 2 as ruled: the width is bounded by geometry, the
//! corner-coverage rule `(1 + 1/sqrt 2) r`, and the proposal is the cut
//! with the most removal per minute. This file now states that truth:
//!
//! 1. The add-door stepover sits on a width rule, the corner-coverage
//!    ceiling (`proposal_limits` names it; P16 deleted the slot step), never
//!    on a force limit.
//! 2. At the drafted depth, every computed force ceiling (the force budget,
//!    power, deflection, the vendor depth table) holds the drafted depth at
//!    every width up to the drafted width: no force limit binds the width
//!    below it.
//!
//! Fixture: the add door drafts a Pocket for a two-flute end mill of D
//! 3.175, 6 and 12 mm (flute 12, 22, 30 mm, stickout flute + 8 mm, stated)
//! on the default machine and material.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_pocket_width_has_no_computed_limit_below_d_p11c
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::session::{AddToolArgs, Command, DraftRequest, ProjectSession};

fn tool(d: f64, flute: f64) -> ToolConfig {
    let mut t = ToolConfig::builder(ToolId::from_raw(0), ToolType::EndMill)
        .diameter(d)
        .shaft_mm(d)
        .shank_mm(d)
        .build();
    t.set_cutting_length(ToolFact::Stated(flute));
    t.set_stickout(ToolFact::Stated(flute + 8.0));
    t.set_flute_count(ToolFact::Stated(2));
    t
}

#[test]
fn no_force_limit_binds_the_pocket_width_below_the_corner_rule_p11c() {
    use rs_cam_core::feeds::envelope::LimitKind;
    let mut checked = 0_usize;
    for (d, flute) in [(3.175, 12.0), (6.0, 22.0), (12.0, 30.0)] {
        let mut s = ProjectSession::new_empty();
        let _ = s
            .apply(Command::AddTool(AddToolArgs {
                tool: Box::new(tool(d, flute)),
            }))
            .expect("the tool");
        let tool_id = s.tools()[0].id;
        let mut op = s
            .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "pocket").tool(tool_id))
            .expect("the draft")
            .config
            .operation;
        let depth = op.depth_per_pass().expect("the add door states a depth");
        let width = op.stepover().expect("the add door states a stepover");
        for field in ["stepover", "depth_per_pass"] {
            if let Some(slot) = rs_cam_core::compute::geometry_default::slot_mut(&mut op, field) {
                *slot = None;
            }
        }
        op.set_spindle_rpm(None);
        let env = SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &s.tools()[0],
            machine: s.machine(),
            material: &s.stock_config().material,
            lut: embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::MatchChart,
            context: SuggestContext::no_project(NoProject::NONE),
        })
        .expect("an envelope");
        // 1. The width sits on a width rule.
        assert!(
            env.proposal_limits
                .iter()
                .any(|k| matches!(k, LimitKind::CornerCoverage)),
            "D {d}: the width {width} sits on {:?}, not on a width rule",
            env.proposal_limits
        );
        // 2. No force ceiling binds the width below the drafted width.
        for k in 1..=20 {
            let ae = width * f64::from(k) / 20.0;
            for limit in &env.boundary {
                let force = matches!(
                    limit.kind,
                    LimitKind::Rigidity
                        | LimitKind::Power
                        | LimitKind::Deflection
                        | LimitKind::VendorDepthTable
                );
                if !force {
                    continue;
                }
                if let Some(ceiling) = limit.ap_ceiling_at(ae) {
                    assert!(
                        ceiling >= depth - 1e-6,
                        "D {d}: {:?} gives ap <= {ceiling} mm at ae {ae} mm, below the drafted \
                         depth {depth} mm: a force limit now binds the width",
                        limit.kind
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 50, "non-vacuity: {checked} ceilings checked");
}
