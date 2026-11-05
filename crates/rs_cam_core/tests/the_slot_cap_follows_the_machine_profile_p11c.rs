//! Feeds contract P11c, D13 item 3 (`planning/feeds_contract_2026-11-03/STATUS.md`,
//! "Lead rulings for P11c"): the slot depth cap is the machine's
//! `RigidityProfile::slot_doc_factor`, not the typed constant
//! `SLOTTING_DOC_CAP`. The slot threshold (`ae > 0.85 D`) stays a
//! machine-neutral geometry rule.
//!
//! Fixture: a 6 mm two-flute end mill in white oak. Two machines differ only
//! in `slot_doc_factor`: the hobby grade default (0.25) and a test value
//! (0.5).
//!
//! Arms:
//!
//! 1. The calculator (Step 4b): a full-width Pocket at a STATED depth of
//!    4 mm warns `SlottingDetected` with `cap_mm = slot_doc_factor x D` and
//!    ships the stated depth (P11b holds: a stated depth ships).
//! 2. The calculator lowers a DEFAULT depth (no stated depth) of a
//!    full-width Profile to `slot_doc_factor x D`.
//! 3. The envelope `Slot` limit is `ap <= slot_doc_factor x D` above
//!    `0.85 D`, and its basis names the machine rigidity with the class
//!    text (D13 item 6), not an F201 row for the cap.
//!
//! Red before P11c: the field does not exist at HEAD f5170da03 (no
//! compile). Mutation red (logged in the P11c report): `slot_rule` forced
//! back to the 0.25 constant fails arms 1-3 on the 0.5 machine.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_slot_cap_follows_the_machine_profile_p11c
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{PocketConfig, ProfileConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::sourced::MachineField;
use rs_cam_core::feeds::suggest::{SuggestContext, feeds_input_for_operation};
use rs_cam_core::feeds::{Basis, FeedsWarning, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::{MachineProfile, RigidityProfile};
use rs_cam_core::material::{Material, WoodSpecies};

const D: f64 = 6.0;

fn tool() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = D;
    tool.set_cutting_length(ToolFact::Stated(22.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn machine(slot_doc_factor: f64) -> MachineProfile {
    MachineProfile {
        rigidity: RigidityProfile {
            slot_doc_factor,
            ..RigidityProfile::default()
        },
        ..MachineProfile::default()
    }
}

fn slot_pocket(depth: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(20.0),
        depth_per_pass: Some(depth),
        stepover: Some(D),
        ..PocketConfig::default()
    })
}

fn slot_warning(op: &OperationConfig, m: &MachineProfile) -> (f64, f64) {
    let (t, mat) = (tool(), oak());
    let input = feeds_input_for_operation(
        op,
        &t,
        &mat,
        m,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    let r = calculate(&input);
    let cap = r
        .warnings
        .iter()
        .find_map(|w| {
            if let FeedsWarning::SlottingDetected { cap_mm, .. } = w {
                Some(*cap_mm)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("no SlottingDetected in {:?}", r.warnings));
    (cap, r.axial_depth_mm)
}

fn machine_fields(b: &Basis) -> Vec<MachineField> {
    match b {
        Basis::Machine(f) => vec![*f],
        Basis::Derived { from, .. } => from.iter().flat_map(machine_fields).collect(),
        Basis::Operator { .. } | Basis::Printed(_) | Basis::Claim(_) | Basis::Rule(_) => Vec::new(),
    }
}

fn laws(b: &Basis) -> Vec<&'static str> {
    match b {
        Basis::Derived { law, from } => std::iter::once(*law)
            .chain(from.iter().flat_map(laws))
            .collect(),
        Basis::Machine(_)
        | Basis::Operator { .. }
        | Basis::Printed(_)
        | Basis::Claim(_)
        | Basis::Rule(_) => Vec::new(),
    }
}

#[test]
fn a_stated_slot_depth_warns_at_the_machine_cap_and_ships_p11c() {
    for factor in [RigidityProfile::default().slot_doc_factor, 0.5] {
        let m = machine(factor);
        let (cap, shipped) = slot_warning(&slot_pocket(4.0), &m);
        assert!(
            (cap - factor * D).abs() < 1e-9,
            "item 3: the slot cap is slot_doc_factor x D = {} mm, got {cap} mm",
            factor * D
        );
        assert!(
            (shipped - 4.0).abs() < 1e-9,
            "P11b: a stated depth ships, got {shipped} mm"
        );
    }
}

#[test]
fn a_default_slot_depth_is_lowered_to_the_machine_cap_p11c() {
    for factor in [RigidityProfile::default().slot_doc_factor, 0.5] {
        let m = machine(factor);
        let op = OperationConfig::Profile(ProfileConfig {
            depth_per_pass: None,
            ..ProfileConfig::default()
        });
        let (cap, shipped) = slot_warning(&op, &m);
        assert!((cap - factor * D).abs() < 1e-9, "cap {cap}");
        assert!(
            (shipped - factor * D).abs() < 1e-9,
            "item 3: a default depth is lowered to slot_doc_factor x D = {} mm, got {shipped}",
            factor * D
        );
    }
}

#[test]
fn the_envelope_slot_limit_reads_the_machine_p11c() {
    for factor in [RigidityProfile::default().slot_doc_factor, 0.5] {
        let m = machine(factor);
        let op = slot_pocket(1.0);
        let (t, mat) = (tool(), oak());
        let env = SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &t,
            machine: &m,
            material: &mat,
            lut: embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::MatchChart,
            context: SuggestContext::default(),
        })
        .expect("a flat end mill on a pocket has an envelope");
        let slot = env.limit(LimitKind::Slot).expect("the slot limit");
        let Edge::ApCeilingAbove { ae_mm, mm } = slot.edge else {
            panic!("the slot limit is a step, got {:?}", slot.edge);
        };
        assert!((mm - factor * D).abs() < 1e-9, "slot ceiling {mm}");
        assert!((ae_mm - 0.85 * D).abs() < 1e-9, "threshold {ae_mm}");
        assert_eq!(machine_fields(&slot.basis), vec![MachineField::Rigidity]);
        assert!(
            laws(&slot.basis).contains(&m.rigidity_basis().text()),
            "D13 item 6: the slot basis names the class text: {:?}",
            slot.basis
        );
        assert!(
            !slot
                .basis
                .unsourced_rules()
                .iter()
                .any(|r| r.name == "feeds.calculate.slotting_doc_cap_d"),
            "the typed cap row is gone"
        );
    }
}
