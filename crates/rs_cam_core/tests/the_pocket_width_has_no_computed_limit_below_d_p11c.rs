//! Feeds contract P11c, D13 item 2 (`planning/feeds_contract_2026-11-03/STATUS.md`,
//! "Lead rulings for P11c"): the ruling asks that the add-door stepover of
//! a roughing Pocket be the K2b envelope proposal's stepover at the op's
//! depth, picked by the computed limits (power, the deflection edge, the
//! chip). P11c did NOT land that change. This file holds the reason as a
//! measured claim (a known finding, an inverted assertion): at the
//! add-door depth, no computed envelope limit bounds the width below the
//! tool diameter, so "the widest width the limits allow" is D, a full slot
//! on every pass.
//!
//! When a computed limit starts to bind the width below D on this fixture
//! (a new width law, or a depth default that the slot or power edge
//! reaches), this test fails. Then item 2 can land as ruled, and this file
//! is deleted.
//!
//! Fixture: the add door (`ProjectSession::draft_toolpath`) drafts a Pocket
//! for a two-flute end mill of D 3.175, 6 and 12 mm (flute 12, 22, 30 mm,
//! stickout flute + 8 mm, stated) on the default machine and material. The
//! envelope is built with the drafted depth and an unstated stepover. For
//! each width sample from 5 % of D to D, every `ap` ceiling of the envelope
//! is at or above the drafted depth.
//!
//! Measured 2026-11-05 (P11c probe, agent scratchpad `p11c_probe_*.log`):
//! the rigidity cap and the slot cap both equal the add-door depth (0.25 D);
//! power and the vendor depth table are above 1 D; deflection reads
//! "no deflection limit below the flute" (and, for a D 3 tool at 40 mm
//! stickout, 2.2 mm at full width, above its 0.75 mm depth).
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
use rs_cam_core::feeds::suggest::SuggestContext;
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
fn no_computed_limit_bounds_the_pocket_width_below_d_p11c() {
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
        if let Some(slot) = rs_cam_core::compute::geometry_default::slot_mut(&mut op, "stepover") {
            *slot = None;
        }
        let env = SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &s.tools()[0],
            machine: s.machine(),
            material: &s.stock_config().material,
            lut: embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::MatchChart,
            context: SuggestContext::default(),
        })
        .expect("an envelope");
        for k in 1..=20 {
            let ae = d * f64::from(k) * 0.05;
            for limit in &env.boundary {
                if let Some(ceiling) = limit.ap_ceiling_at(ae) {
                    assert!(
                        ceiling >= depth - 1e-9,
                        "a computed limit now binds the width: {:?} gives ap <= {ceiling} mm \
                         at ae {ae} mm (D {d}), below the add-door depth {depth} mm. \
                         P11c item 2 can land; delete this file.",
                        limit.kind
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 100, "non-vacuity: {checked} ceilings checked");
}
