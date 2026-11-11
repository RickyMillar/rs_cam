//! Feeds contract P13c (operator ruling O1, 2026-11-05): the roughing depth
//! of the Pocket and Profile families is the depth where the peak
//! engaged-edge force meets the machine force budget.
//!
//! The claims, on the FM1 tools (2 flutes, flute max(3 D, 12), stickout
//! flute + 8) and the default (hobby) machine:
//!
//! 1. The envelope `Rigidity` limit of a Pocket is a curve over the width.
//!    At each sample with an edge, the peak force at the column chip is at
//!    most the budget at the edge depth and over it 0.01 mm deeper.
//! 2. A Profile (ae = D) has one `Rigidity` ceiling that meets the budget.
//! 3. A tiny budget ships the minimum depth with a warning (Q6: never
//!    refuse): the law says so and the envelope notes carry the warning.
//! 4. Retired in P29-4 (ruling R1, 2026-11-11): the Adaptive family kept
//!    `adaptive_doc_factor x D`. It reads the force budget now.
//! 5. A material with no force line (plywood) states the abstention.
//!
//! Red before P13c (HEAD 4efcb4eaa): `feeds::force_budget` and
//! `RigidityProfile::force_budget_n` do not exist (no compile); the
//! `Rigidity` limit was the ceiling `doc_roughing_factor x D`.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_roughing_depth_meets_the_force_budget_p13c
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::geometry_default::slot_mut;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::force_budget::{
    FORCE_BUDGET_FLOOR_LAW, FORCE_BUDGET_FLOOR_NOTE, FORCE_BUDGET_LAW, NO_FORCE_LINE, peak_force_n,
};
use rs_cam_core::feeds::suggest::SuggestContext;
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

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

fn materials() -> [(&'static str, Material); 3] {
    [
        (
            "softwood",
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
        ),
        (
            "hardwood",
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
        ),
        (
            "mdf",
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
    ]
}

/// The op of `kind` with its geometry fields unset and no stated speeds:
/// the envelope reads the grid chip.
fn op(kind: OperationType) -> OperationConfig {
    let mut op = OperationConfig::new_default(kind);
    for field in ["stepover", "depth_per_pass"] {
        if let Some(slot) = slot_mut(&mut op, field) {
            *slot = None;
        }
    }
    op.set_spindle_rpm(None);
    op
}

fn envelope(
    op: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
) -> SafeEnvelope {
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool,
        machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("the FM1 cell is backed")
}

/// The chip of the grid column at `ae` (the shallowest sample).
fn column_chip(env: &SafeEnvelope, ae: f64) -> Option<f64> {
    let j = env.speeds.aes.iter().position(|w| (w - ae).abs() < 1e-12)?;
    env.speeds.at(0, j).map(|p| p.chip.get().0)
}

#[test]
fn the_pocket_rigidity_edge_meets_the_budget_at_each_width_p13c() {
    let machine = MachineProfile::default();
    let budget = machine.rigidity.force_budget_n;
    let mut checked = 0_usize;
    for d in [3.175, 6.0] {
        let t = tool(d);
        let cutter = rs_cam_core::compute::cutter::build_cutter(&t).expect("a cutter");
        for (name, m) in materials() {
            let env = envelope(&op(OperationType::Pocket), &t, &machine, &m);
            let l = env.limit(LimitKind::Rigidity).expect("a Rigidity limit");
            assert_eq!(l.law, FORCE_BUDGET_LAW, "{name} D {d}");
            let Edge::ApCurve { points } = &l.edge else {
                panic!("{name} D {d}: the Pocket edge is a curve, got {:?}", l.edge)
            };
            let top = *env.speeds.aps.last().expect("a depth span");
            for &[ae, ap] in points {
                let Some(fz) = column_chip(&env, ae) else {
                    continue; // an exact sample off the grid
                };
                if ap >= top {
                    continue; // no root below the flute
                }
                let at = peak_force_n(&cutter, &m, ap, ae, fz).expect("a force");
                let past = peak_force_n(&cutter, &m, ap + 0.01, ae, fz).expect("a force");
                assert!(
                    at <= budget * (1.0 + 1e-9),
                    "{name} D {d} ae {ae}: {at} N at {ap} mm over {budget}"
                );
                assert!(
                    past > budget,
                    "{name} D {d} ae {ae}: {past} N at {ap}+0.01 mm not past {budget}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "non-vacuity: {checked} edge samples");
}

#[test]
fn the_profile_depth_meets_the_budget_at_full_width_p13c() {
    let machine = MachineProfile::default();
    let budget = machine.rigidity.force_budget_n;
    let t = tool(6.0);
    let cutter = rs_cam_core::compute::cutter::build_cutter(&t).expect("a cutter");
    for (name, m) in materials() {
        let env = envelope(&op(OperationType::Profile), &t, &machine, &m);
        let l = env.limit(LimitKind::Rigidity).expect("a Rigidity limit");
        let Edge::ApCeiling { mm } = l.edge else {
            panic!("{name}: a Profile has one ceiling, got {:?}", l.edge)
        };
        let fz = column_chip(&env, 6.0).expect("the slot column");
        let at = peak_force_n(&cutter, &m, mm, 6.0, fz).expect("a force");
        let past = peak_force_n(&cutter, &m, mm + 0.01, 6.0, fz).expect("a force");
        assert!(at <= budget * (1.0 + 1e-9), "{name}: {at} at {mm}");
        assert!(past > budget, "{name}: {past} at {mm}+0.01");
    }
}

#[test]
fn a_tiny_budget_ships_the_minimum_depth_with_a_warning_p13c() {
    let mut machine = MachineProfile::default();
    machine.rigidity.force_budget_n = 0.5;
    let t = tool(6.0);
    let (_, m) = &materials()[1];
    let env = envelope(&op(OperationType::Pocket), &t, &machine, m);
    let l = env.limit(LimitKind::Rigidity).expect("a Rigidity limit");
    assert_eq!(l.law, FORCE_BUDGET_FLOOR_LAW);
    assert!(
        env.notes.iter().any(|n| n == FORCE_BUDGET_FLOOR_NOTE),
        "{:?}",
        env.notes
    );
    let floor = env
        .boundary
        .iter()
        .find_map(|l| match (l.kind, &l.edge) {
            (LimitKind::EngagementFloor, Edge::ApFloor { mm }) => Some(*mm),
            _ => None,
        })
        .expect("the depth floor");
    let p = env.proposal.get().point().expect("a proposed depth");
    assert!(
        (p.ap_mm - floor).abs() < 1e-12,
        "the minimum depth ships: {p:?}"
    );
}

#[test]
fn a_material_with_no_force_line_states_the_abstention_p13c() {
    let machine = MachineProfile::default();
    let t = tool(6.0);
    let m = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    let env = envelope(&op(OperationType::Pocket), &t, &machine, &m);
    let l = env.limit(LimitKind::Rigidity).expect("the entry stays");
    assert_eq!(
        l.edge,
        Edge::Unmodelled {
            reason: NO_FORCE_LINE
        }
    );
}

/// The P13c report numbers: the budget per chart row, the median, and the
/// pick (width, depth, binding limits) on the FM1 Pocket, Profile and
/// Adaptive cells, with the force-budget depth at the Adaptive width.
#[test]
#[ignore = "instrument: prints the P13c report numbers"]
#[expect(
    clippy::print_stdout,
    reason = "an instrument prints its measurements for the reader"
)]
fn p13c_report_instrument() {
    use rs_cam_core::feeds::force_budget::{HOBBY_SLOT_POINTS, depth_at_budget, hobby_budget};
    let mut peaks: Vec<f64> = Vec::new();
    for p in &HOBBY_SLOT_POINTS {
        let f = p.peak_force_n().expect("a force");
        println!("row {} peak {f:.4} N", p.id);
        peaks.push(f);
    }
    peaks.sort_by(f64::total_cmp);
    println!(
        "budget {:.4} N, median {:.4} N",
        hobby_budget().force_n,
        0.5 * (peaks[2] + peaks[3])
    );
    // The G-CHIPVERDICT heavy box: a stated full-width 2 x D cut at
    // 3000 mm/min and 18 000 rpm (Makita preset, hardwood, D 6, stickout
    // 3 D), and the point "Apply all" picks inside it.
    for (label, m) in [
        ("makita", MachineProfile::shapeoko_makita()),
        ("default", MachineProfile::default()),
    ] {
        let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
        t.diameter = 6.0;
        t.set_flute_count(ToolFact::Stated(2));
        t.set_stickout(ToolFact::Stated(18.0));
        let mut o = OperationConfig::new_default(OperationType::Pocket);
        o.set_spindle_rpm(Some(18_000));
        o.set_feed_rate(3_000.0);
        o.set_stepover(6.0);
        let _ = o.set_depth_per_pass(12.0);
        let env = envelope(&o, &t, &m, &materials()[1].1);
        println!(
            "heavy box {label}: proposal {:?} limits {:?}",
            env.proposal.get(),
            env.proposal_limits
        );
    }
    let machine = MachineProfile::default();
    for d in [3.175, 6.0] {
        let t = tool(d);
        let cutter = rs_cam_core::compute::cutter::build_cutter(&t).expect("a cutter");
        for (name, m) in materials() {
            for kind in [
                OperationType::Pocket,
                OperationType::Profile,
                OperationType::Adaptive,
            ] {
                let env = envelope(&op(kind), &t, &machine, &m);
                let p = env.proposal.get().point().expect("a proposed depth");
                let vf = env
                    .pick(rs_cam_core::feeds::envelope::PickRequest::Proposal)
                    .result()
                    .feed_rate_mm_min;
                println!(
                    "{kind:?} D {d} {name}: ae {:.4} ({:.3} D) ap {:.4} ({:.3} D) vf {vf:.0} MRR {:.0} limits {:?}",
                    p.ae_mm,
                    p.ae_mm / d,
                    p.ap_mm,
                    p.ap_mm / d,
                    p.ae_mm * p.ap_mm * vf,
                    env.proposal_limits
                );
                if kind == OperationType::Adaptive {
                    let at = env
                        .pick(rs_cam_core::feeds::envelope::PickRequest::Point {
                            ap_mm: 0.05,
                            ae_mm: p.ae_mm,
                        })
                        .result()
                        .clone();
                    let fz = at.feed_rate_mm_min / (at.rpm * 2.0);
                    let b =
                        depth_at_budget(&cutter, &m, p.ae_mm, fz, machine.rigidity.force_budget_n);
                    println!(
                        "  budget depth at the adaptive width {:.3}: {b:?} (factor cap {:.3})",
                        p.ae_mm,
                        machine.rigidity.adaptive_doc_factor * d
                    );
                    for frac in [0.1, 0.2, 0.25, 0.35] {
                        let b = depth_at_budget(
                            &cutter,
                            &m,
                            frac * d,
                            fz,
                            machine.rigidity.force_budget_n,
                        );
                        println!("  budget depth at ae {frac} D (fz {fz:.4}): {b:?}");
                    }
                }
            }
        }
    }
}
