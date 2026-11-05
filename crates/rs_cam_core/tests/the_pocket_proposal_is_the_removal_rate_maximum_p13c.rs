//! Feeds contract P13c (operator rulings O3 and O4, 2026-11-05): the
//! envelope proposal of a roughing Pocket with a free width and depth is
//! the cut with the most removal per minute inside every limit, and the
//! corner-coverage rule `(1 + 1/sqrt 2) r` bounds its width.
//!
//! The claims, on the FM1 flat end mills (D 3.175 and 6, 2 flutes) in
//! softwood, hardwood and MDF on the default machine, with the stepover and
//! the depth unset and no stated speeds:
//!
//! 1. The proposal is inside the envelope.
//! 2. Its width is at most `(1 + 1/sqrt 2) / 2 x D` (derived here from the
//!    geometry, not typed).
//! 3. Its removal rate `ae x ap x vf` (vf: the feed the envelope ships at
//!    the point) is at least that of every point of a 40 x 40 grid that
//!    the envelope places inside, less 0.5 % (the grid places a point on
//!    curves interpolated between eight width samples; the proposal reads
//!    the force budget exactly).
//!
//! This file uses only the envelope API of HEAD 4efcb4eaa, so it compiles
//! there and fails on claim 3 (the proposal was the family default
//! 0.35 D x 0.70 D lowered to the caps).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_pocket_proposal_is_the_removal_rate_maximum_p13c
//! ```

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::geometry_default::slot_mut;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{CutPoint, EnvelopeInput, PickRequest, Placement, SafeEnvelope};
use rs_cam_core::feeds::suggest::SuggestContext;
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, SheetGoodKind, WoodSpecies};

fn tool(d: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = d;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d.max(3.0)));
    t.shaft_diameter = d.max(3.0);
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

fn removal(env: &SafeEnvelope, p: CutPoint) -> f64 {
    let vf = env
        .pick(PickRequest::Point {
            ap_mm: p.ap_mm,
            ae_mm: p.ae_mm,
        })
        .result()
        .feed_rate_mm_min;
    p.ae_mm * p.ap_mm * vf
}

#[test]
fn the_pocket_proposal_is_the_most_removal_inside_the_limits_p13c() {
    // The corner rule from the geometry: an inner offset corner at
    // (r + s, r + s) covers the outer loop's uncut corner (2r, 2r) when
    // (s - r) sqrt 2 <= r.
    let corner = |d: f64| {
        let r = d / 2.0;
        r + r / std::f64::consts::SQRT_2
    };
    let machine = MachineProfile::default();
    let materials = [
        Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        },
    ];
    let mut compared = 0_usize;
    for d in [3.175, 6.0] {
        let t = tool(d);
        for m in &materials {
            let mut op = OperationConfig::new_default(OperationType::Pocket);
            for field in ["stepover", "depth_per_pass"] {
                if let Some(slot) = slot_mut(&mut op, field) {
                    *slot = None;
                }
            }
            op.set_spindle_rpm(None);
            let env = SafeEnvelope::for_cut(EnvelopeInput {
                operation: &op,
                tool: &t,
                machine: &machine,
                material: m,
                lut: embedded_vendor_lut(),
                spindle_strategy: SpindleStrategy::default(),
                context: SuggestContext::default(),
            })
            .expect("the FM1 cell is backed");
            let p = env.proposal.get();
            let what = format!("{m:?} D {d}");
            assert!(
                matches!(env.placement_of(p), Placement::Inside { .. }),
                "{what}: the proposal {p:?} is outside: {:?}",
                env.placement_of(p)
            );
            assert!(
                p.ae_mm <= corner(d) + 1e-9,
                "{what}: width {} over the corner bound {}",
                p.ae_mm,
                corner(d)
            );
            let best = removal(&env, p);
            let top = *env.speeds.aps.last().expect("a depth span");
            for i in 1..=40 {
                for j in 1..=40 {
                    let q = CutPoint {
                        ap_mm: top * f64::from(i) / 40.0,
                        ae_mm: d * f64::from(j) / 40.0,
                    };
                    if !matches!(env.placement_of(q), Placement::Inside { .. }) {
                        continue;
                    }
                    let r = removal(&env, q);
                    assert!(
                        best >= r * (1.0 - 5e-3),
                        "{what}: the proposal {p:?} removes {best} mm3/min, the inside \
                         point {q:?} removes {r}"
                    );
                    compared += 1;
                }
            }
        }
    }
    assert!(compared > 100, "non-vacuity: {compared} inside points");
}
