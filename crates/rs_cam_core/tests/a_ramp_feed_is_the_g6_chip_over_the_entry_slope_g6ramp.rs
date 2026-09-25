//! G6 ramp — the helix and ramp entry feed is the G6 axial chip over the
//! entry slope, capped at the cut feed.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/RAMP_PLAN.md` (§1F,
//! §2 step 3) and its "Orchestrator decisions".
//!
//! The rule: `ramp feed = min(cut feed, axial chip x RPM x Z / tan θ)`,
//! rounded down to 1 mm/min. The axial chip is the G6 drill claim
//! (`feeds::extrapolation::drill`): the tool's Amana Spektra side row / Z.
//! θ is the entry slope of the operation that ships: `tan(angle)` for a
//! ramp, `pitch / (2π r)` for a helix. The RPM and the cut feed are the
//! shipped values. With no G6 chip the ramp holds its vertical rate at the
//! plunge (G10 Q2, `a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10`).
//! A straight plunge or an unknown entry writes `None`, and the entry uses
//! the plunge rate.
//!
//! The rows the claim reads (`data/vendor_lut/observations/amana_flat_end.json`,
//! all `amana_spektra_spiral_plunge_v24`, filed under (Pocket, Roughing),
//! `rpm_nominal` 18 000). Each cell is the side chip -> the side chip / 2:
//!
//! | Tool | Softwood (600) | Hardwood (1450) | Plywood (Baltic birch) | MDF |
//! |---|---|---|---|---|
//! | 6 mm 2F | `amana-flat-softwood-pocket-6000-2f-spektra` 0.127 -> 0.0635 | `amana-flat-hardwood-pocket-6000-2f-spektra` 0.127 -> 0.0635 | `amana-flat-plywood-hardwood-pocket-6000-2f-spektra` 0.127 -> 0.0635 | `amana-flat-mdf-pocket-6000-2f-spektra` 0.1524 -> 0.0762 |
//! | 3.175 mm 2F | `amana-flat-softwood-pocket-3175-2f-spektra` 0.1016 -> 0.0508 | `amana-flat-hardwood-pocket-3175-2f-spektra` 0.1016 -> 0.0508 | `amana-flat-plywood-hardwood-pocket-3175-2f-spektra` 0.1016 -> 0.0508 | `amana-flat-mdf-pocket-3175-2f-spektra` 0.127 -> 0.0635 |
//!
//! The worked example at the chart's 18 000 RPM (6 mm 2F flat, generic
//! softwood). The Suggest pipeline can ship a lower RPM (9842 on the
//! default pocket), so the cases derive the expected values from the RPM
//! and the feed that ship:
//!
//! - axial chip = 0.127 / 2 = 0.0635 mm/tooth; vertical rate = 0.0635 x
//!   18 000 x 2 = 2286 mm/min (the printed Ramp Down of 90 in/min x 25.4);
//! - cut feed = 0.127 x 18 000 x 2 x tier 1.0 = 4572 mm/min;
//! - helix r 2 mm, pitch 1 mm: tan θ = 1 / (2π x 2) = 0.079577, θ = 4.55°;
//!   chip term = 2286 / 0.079577 = 28 726 mm/min; ramp feed = 4572, the cut
//!   feed;
//! - ramp 3°: tan θ = 0.052408; chip term = 43 619 mm/min; ramp feed = 4572;
//! - steep check (c): a 60° ramp puts the chip term under the cut feed.
//!   It reads the shipped RPM; the drill RPM cap (14 000) would give
//!   0.0635 x 14 000 x 2 / tan 60° = 1026.
//!
//! The arms:
//!
//! - (a) and (b) the worked example, helix and ramp 3°;
//! - (c) the 60° ramp: the chip term wins, at the shipped RPM;
//! - (d) Adaptive3d reads its own helix (0.3 x D, pitch 2): θ 10.03°;
//! - (e) the chip table above;
//! - (f) every entry fallback writes `None` and states its reason, and a
//!   cell with no G6 claim states why;
//! - (g) a drill cycle files no record and has no field;
//! - (h) a cut-geometry apply does not write the ramp, a speeds apply does;
//! - (i) Suggest and the apply funnel give one number;
//! - (j) the value rounds down;
//! - (k) the provenance stamps.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::operation_configs::Adaptive3dEntryStyle;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::ramp::{EntrySource, RampArm, RampBasis, RampFallback, SourcedRamp};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, SuggestContext, SuggestForOperationInput, SuggestWarning,
    SuggestedParams, apply, apply_cut_geometry_to_op, apply_speeds_to_op,
    feeds_input_for_operation, feeds_preview_for_operation, feeds_result_for_operation,
    suggest_for_operation,
};
use rs_cam_core::feeds::{
    FeedsProvenance, ProvenanceSource, RampFeed, SpindleStrategy, ValueProvenance, calculate,
    embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily, PlywoodGrade, SheetGoodKind, WoodSpecies};

/// The softwood 6 mm 2-flute side row.
const SOFTWOOD_6: &str = "amana-flat-softwood-pocket-6000-2f-spektra";

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn plywood() -> Material {
    Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    }
}

fn mdf() -> Material {
    Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    }
}

fn tool_of(kind: ToolType, diameter: f64, flutes: u32) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = flutes;
    t.cutting_length = (diameter * 4.0).max(12.0);
    t.shank_diameter = diameter.max(3.175);
    t.shaft_diameter = diameter.max(3.175);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = 0.5;
        t.corner_radius_mm = 0.5;
    }
    t
}

fn flat(diameter: f64, flutes: u32) -> ToolConfig {
    tool_of(ToolType::EndMill, diameter, flutes)
}

/// The generic router with a 6000 mm/min cutting ceiling.
fn open_router() -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.max_cutting_feed_mm_min = Some(6000.0);
    m
}

fn helix_dressup(radius_mm: f64, pitch_mm: f64) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        helix_radius: Some(radius_mm),
        helix_pitch: pitch_mm,
        ..DressupConfig::default()
    }
}

fn ramp_dressup(angle_deg: f64) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: angle_deg,
        ..DressupConfig::default()
    }
}

fn no_entry() -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::None,
        ..DressupConfig::default()
    }
}

fn pocket() -> OperationConfig {
    OperationConfig::new_default(OperationType::Pocket)
}

fn context(dressups: Option<&DressupConfig>) -> SuggestContext<'_> {
    SuggestContext {
        dressups,
        ..SuggestContext::default()
    }
}

fn suggest(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    dressups: Option<&DressupConfig>,
) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation,
        tool,
        machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: context(dressups),
    })
    .unwrap_or_else(|e| panic!("Suggest must serve {:?}: {e}", operation.op_type()))
}

/// The one `RampFeed` record of an apply, if any.
fn records(warnings: &[SuggestWarning]) -> Vec<(Option<f64>, &RampFeed)> {
    warnings
        .iter()
        .filter_map(|w| match w {
            SuggestWarning::RampFeed {
                from_mm_min,
                record,
                ..
            } => Some((*from_mm_min, record)),
            _ => None,
        })
        .collect()
}

fn record(s: &SuggestedParams) -> &RampFeed {
    let all = records(&s.warnings);
    assert_eq!(all.len(), 1, "one RampFeed record per apply: {all:?}");
    all[0].1
}

fn sourced(r: &RampFeed) -> &SourcedRamp {
    match r {
        RampFeed::Sourced(s) => s,
        RampFeed::PlungeSlope(s) => panic!("expected a sourced ramp, got {s:?}"),
        RampFeed::NoEntryFeed { reason, .. } => panic!("expected a sourced ramp, got {reason:?}"),
    }
}

fn fallback(r: &RampFeed) -> RampFallback {
    match r {
        RampFeed::NoEntryFeed { reason, .. } => *reason,
        RampFeed::Sourced(s) => panic!("expected the plunge rate, got {s:?}"),
        RampFeed::PlungeSlope(s) => panic!("expected the plunge rate, got {s:?}"),
    }
}

/// The sourced half of the calculator for one pocket cell (no validation,
/// so a tool that Suggest refuses on a pocket still reports its basis).
fn basis(tool: &ToolConfig, material: &Material) -> RampBasis {
    let machine = MachineProfile::generic_wood_router();
    let input = feeds_input_for_operation(
        &pocket(),
        tool,
        material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    calculate(&input).ramp
}

/// The chip term and the ramp feed that the rule gives at the RPM and the
/// feed that ship: `min(feed, 0.0635 x rpm x 2 / tan θ)`, rounded down.
fn expected(s: &SuggestedParams, tan_theta: f64) -> (f64, f64) {
    let rpm = f64::from(
        s.operation
            .spindle_rpm()
            .expect("the speeds apply writes the RPM"),
    );
    let chip_term = 0.0635 * rpm * 2.0 / tan_theta;
    (chip_term, s.operation.feed_rate().min(chip_term).floor())
}

/// (a) The default helix (r 2, pitch 1): θ 4.55°, and the cut feed wins.
/// The chip term reads the RPM that ships (the worked example above uses
/// the chart's 18 000; the Suggest pipeline can ship a lower RPM).
#[test]
fn the_helix_ships_the_cut_feed_at_the_shipped_rpm_g6ramp() {
    let dressups = helix_dressup(2.0, 1.0);
    let s = suggest(
        &pocket(),
        &flat(6.0, 2),
        &softwood(),
        &open_router(),
        Some(&dressups),
    );
    let r = record(&s);
    let ramp = sourced(r);
    let rpm = f64::from(s.operation.spindle_rpm().expect("rpm"));
    assert!((ramp.rpm - rpm).abs() < 1e-9, "{} vs {rpm}", ramp.rpm);
    assert!(
        (ramp.axial_chip_mm - 0.0635).abs() < 1e-9,
        "{}",
        ramp.axial_chip_mm
    );
    assert_eq!(ramp.drill.side_row, SOFTWOOD_6);
    assert_eq!(ramp.source, EntrySource::Dressup);
    assert!((ramp.theta_deg - 4.55).abs() < 0.01, "{}", ramp.theta_deg);
    let (chip_term, value) = expected(&s, 1.0 / (std::f64::consts::TAU * 2.0));
    assert!(
        (ramp.chip_term_mm_min - chip_term).abs() < 1e-6,
        "{} vs {chip_term}",
        ramp.chip_term_mm_min
    );
    assert_eq!(ramp.arm, RampArm::CutFeed);
    assert_eq!(r.value(), Some(value));
    assert_eq!(r.value(), Some(s.operation.feed_rate()));
    assert_eq!(s.operation.ramp_feed_rate(), r.value());
    let (face, _) = r.card_text();
    assert!(
        face.contains("cut feed") && face.contains("4.55°"),
        "{face}"
    );
}

/// (b) The same cell with a 3° ramp.
#[test]
fn the_three_degree_ramp_ships_the_cut_feed_g6ramp() {
    let dressups = ramp_dressup(3.0);
    let s = suggest(
        &pocket(),
        &flat(6.0, 2),
        &softwood(),
        &open_router(),
        Some(&dressups),
    );
    let r = record(&s);
    let ramp = sourced(r);
    let (chip_term, _) = expected(&s, 3.0_f64.to_radians().tan());
    assert!(
        (ramp.chip_term_mm_min - chip_term).abs() < 1e-6,
        "{} vs {chip_term}",
        ramp.chip_term_mm_min
    );
    assert_eq!(ramp.arm, RampArm::CutFeed);
    assert_eq!(s.operation.ramp_feed_rate(), Some(s.operation.feed_rate()));
}

/// (c) A 60° ramp: the chip term is under the cut feed, so it wins. It
/// reads the RPM that ships, not the drill RPM cap (14 000).
#[test]
fn a_steep_ramp_ships_the_chip_term_at_the_shipped_rpm_g6ramp() {
    let dressups = ramp_dressup(60.0);
    let s = suggest(
        &pocket(),
        &flat(6.0, 2),
        &softwood(),
        &MachineProfile::generic_wood_router(),
        Some(&dressups),
    );
    let r = record(&s);
    let ramp = sourced(r);
    let tan60 = 60.0_f64.to_radians().tan();
    let (chip_term, value) = expected(&s, tan60);
    assert!(chip_term < s.operation.feed_rate(), "{chip_term}");
    assert_eq!(ramp.arm, RampArm::ChipTerm);
    assert_eq!(r.value(), Some(value));
    assert_eq!(s.operation.ramp_feed_rate(), Some(value));
    let drill_cap = (0.0635 * 14_000.0 * 2.0 / tan60).floor();
    if s.operation.spindle_rpm() != Some(14_000) {
        assert_ne!(
            r.value(),
            Some(drill_cap),
            "the chip term reads the shipped RPM, not the drill RPM cap"
        );
    }
    assert!(ramp.value_mm_min < s.operation.feed_rate());
}

/// (d) Adaptive3d reads its own helix: radius 0.3 x 6 mm, pitch 2 mm.
#[test]
fn adaptive3d_reads_its_own_helix_g6ramp() {
    let mut op = OperationConfig::new_default(OperationType::Adaptive3d);
    if let OperationConfig::Adaptive3d(cfg) = &mut op {
        cfg.entry_style = Adaptive3dEntryStyle::Helix;
        cfg.helix_radius_factor = 0.3;
        cfg.helix_pitch = 2.0;
    }
    let s = suggest(&op, &flat(6.0, 2), &softwood(), &open_router(), None);
    let ramp = sourced(record(&s));
    assert_eq!(ramp.source, EntrySource::Adaptive3d);
    assert!((ramp.theta_deg - 10.03).abs() < 0.01, "{}", ramp.theta_deg);
    assert_eq!(s.operation.ramp_feed_rate(), Some(ramp.value_mm_min));
}

/// (e) The chip table: each cell reads its side row / 2.
#[test]
fn the_chip_is_the_side_row_over_z_g6ramp() {
    for (d, material, name, row, chip) in [
        (
            3.175,
            softwood(),
            "softwood",
            "amana-flat-softwood-pocket-3175-2f-spektra",
            0.0508,
        ),
        (
            3.175,
            hardwood(),
            "hardwood",
            "amana-flat-hardwood-pocket-3175-2f-spektra",
            0.0508,
        ),
        (
            6.0,
            mdf(),
            "mdf",
            "amana-flat-mdf-pocket-6000-2f-spektra",
            0.0762,
        ),
        (
            3.175,
            mdf(),
            "mdf",
            "amana-flat-mdf-pocket-3175-2f-spektra",
            0.0635,
        ),
        (
            6.0,
            plywood(),
            "plywood",
            "amana-flat-plywood-hardwood-pocket-6000-2f-spektra",
            0.0635,
        ),
    ] {
        match basis(&flat(d, 2), &material) {
            RampBasis::Sourced {
                axial_chip_mm,
                drill,
                ..
            } => {
                assert!(
                    (axial_chip_mm - chip).abs() < 1e-9,
                    "{name} {d} mm: chip {axial_chip_mm}, want {chip}"
                );
                assert_eq!(drill.side_row, row, "{name} {d} mm");
            }
            RampBasis::NoChip { reason } => {
                panic!("{name} {d} mm: expected a G6 chip, got {reason:?}")
            }
        }
    }
}

/// (f) Every entry fallback writes `None` and states its reason. A cell
/// with no G6 claim states why (its ramp is the G10 Q2 plunge slope).
#[test]
fn every_fallback_writes_none_and_says_why_g6ramp() {
    let machine = open_router();
    // The entry is a straight plunge.
    let off = no_entry();
    let s = suggest(&pocket(), &flat(6.0, 2), &softwood(), &machine, Some(&off));
    assert_eq!(fallback(record(&s)), RampFallback::EntryOff);
    assert_eq!(s.operation.ramp_feed_rate(), None);
    // A dressup operation with no dressups in the context.
    let s = suggest(&pocket(), &flat(6.0, 2), &softwood(), &machine, None);
    assert_eq!(fallback(record(&s)), RampFallback::EntryUnknown);
    assert_eq!(s.operation.ramp_feed_rate(), None);
    let (face, _) = record(&s).card_text();
    assert!(face.contains("the plunge rate"), "{face}");

    // No G6 claim: a bull, a ball, a flat outside the range, a 4-flute flat,
    // and a flat in acrylic.
    let acrylic = Material::Plastic {
        family: PlasticFamily::Acrylic,
    };
    for (name, tool, material) in [
        ("bull 6 mm", tool_of(ToolType::BullNose, 6.0, 2), softwood()),
        ("ball 6 mm", tool_of(ToolType::BallNose, 6.0, 2), softwood()),
        // 15.875 mm (5/8 in): above the widened 3.0-12.7 mm range (the G6
        // Spektra load, 2026-09-25); 6.35 mm is inside it now.
        ("flat 15.875 mm", flat(15.875, 2), softwood()),
        ("flat 6 mm 4F", flat(6.0, 4), softwood()),
        ("flat 6 mm acrylic", flat(6.0, 2), acrylic),
    ] {
        let b = basis(&tool, &material);
        let RampBasis::NoChip { reason } = &b else {
            panic!("{name}: expected no G6 claim, got {b:?}");
        };
        assert!(
            matches!(reason, RampFallback::NoDrillClaim { .. }),
            "{name}: {reason:?}"
        );
        assert!(reason.text().contains("G6"), "{name}: {}", reason.text());
    }
}

/// (g) A drill cycle has no ramp field: no record, and the field stays
/// `None`.
#[test]
fn a_drill_cycle_files_no_ramp_record_g6ramp() {
    for op_type in [OperationType::Drill, OperationType::AlignmentPinDrill] {
        let op = OperationConfig::new_default(op_type);
        let dressups = DressupConfig::for_op(op_type);
        let s = suggest(
            &op,
            &flat(6.0, 2),
            &softwood(),
            &open_router(),
            Some(&dressups),
        );
        assert!(
            records(&s.warnings).is_empty(),
            "{op_type:?}: a drill files no RampFeed record"
        );
        assert_eq!(s.operation.ramp_feed_rate(), None, "{op_type:?}");
        assert_eq!(s.provenance.ramp_feed_rate, None, "{op_type:?}");
    }
}

/// (h) A cut-geometry apply leaves the ramp alone; a speeds apply writes it.
#[test]
fn only_a_speeds_apply_writes_the_ramp_g6ramp() {
    let machine = open_router();
    let tool = flat(6.0, 2);
    let material = softwood();
    let dressups = helix_dressup(2.0, 1.0);
    let op = pocket();
    let result = feeds_result_for_operation(
        &op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    )
    .expect("the pocket cell ships");
    let pass_role = op.feeds_style().1;

    let mut cut = op.clone();
    let mut prov = FeedsProvenance::default();
    let warnings = apply_cut_geometry_to_op(
        &mut cut,
        &mut prov,
        &result,
        &tool,
        &machine,
        &material,
        pass_role,
        context(Some(&dressups)),
    );
    assert_eq!(cut.ramp_feed_rate(), op.ramp_feed_rate());
    assert!(records(&warnings).is_empty());
    assert_eq!(prov.ramp_feed_rate, None);

    let mut speeds = op.clone();
    let mut prov = FeedsProvenance::default();
    let warnings = apply_speeds_to_op(
        &mut speeds,
        &mut prov,
        &result,
        &tool,
        &machine,
        &material,
        pass_role,
        context(Some(&dressups)),
    );
    let all = records(&warnings);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].0, op.ramp_feed_rate(), "from is the value before");
    assert!(speeds.ramp_feed_rate().is_some());
    assert_eq!(speeds.ramp_feed_rate(), all[0].1.value());
}

/// (i) One number: the Suggest record and the value the apply funnel writes
/// with the same context.
#[test]
fn suggest_and_apply_write_one_ramp_number_g6ramp() {
    let machine = open_router();
    let tool = flat(6.0, 2);
    let material = softwood();
    for dressups in [
        helix_dressup(2.0, 1.0),
        ramp_dressup(3.0),
        ramp_dressup(30.0),
    ] {
        let op = pocket();
        let s = suggest(&op, &tool, &material, &machine, Some(&dressups));
        let preview = feeds_preview_for_operation(
            &op,
            &tool,
            &material,
            &machine,
            embedded_vendor_lut(),
            SpindleStrategy::MatchChart,
        );
        let rec = preview.applicable().expect("the pocket cell validates");
        let mut applied = op.clone();
        let mut prov = FeedsProvenance::default();
        apply(
            &rec,
            ApplyScope::Both,
            &mut applied,
            &mut prov,
            ApplyContext {
                tool: &tool,
                machine: &machine,
                material: &material,
                pass_role: op.feeds_style().1,
                suggest: context(Some(&dressups)),
            },
        );
        assert!(applied.ramp_feed_rate().is_some());
        assert_eq!(record(&s).value(), applied.ramp_feed_rate());
        assert_eq!(s.operation.ramp_feed_rate(), applied.ramp_feed_rate());
    }
}

/// (j) The value rounds down: at or below min(feed, chip term), and less
/// than 1 mm/min under it.
#[test]
fn the_ramp_rounds_down_g6ramp() {
    for (machine, dressups) in [
        (open_router(), helix_dressup(2.0, 1.0)),
        (open_router(), ramp_dressup(3.0)),
        (MachineProfile::generic_wood_router(), ramp_dressup(30.0)),
        (MachineProfile::generic_wood_router(), ramp_dressup(45.0)),
    ] {
        let s = suggest(
            &pocket(),
            &flat(6.0, 2),
            &softwood(),
            &machine,
            Some(&dressups),
        );
        let ramp = sourced(record(&s));
        let bound = ramp.cut_feed_mm_min.min(ramp.chip_term_mm_min);
        assert!(
            ramp.value_mm_min <= bound,
            "{} > {bound}",
            ramp.value_mm_min
        );
        assert!(
            ramp.value_mm_min > bound - 1.0,
            "{} <= {bound} - 1",
            ramp.value_mm_min
        );
        assert!(ramp.value_mm_min <= s.operation.feed_rate());
    }
}

/// (k) The provenance: a chip term stamps the G6 side row, the cut feed
/// copies the feed stamp, the plunge fallback copies the plunge stamp, and
/// a later hand edit reads Manual.
#[test]
fn the_ramp_provenance_names_its_source_g6ramp() {
    let tool = flat(6.0, 2);
    let material = softwood();

    let steep = ramp_dressup(30.0);
    let s = suggest(
        &pocket(),
        &tool,
        &material,
        &MachineProfile::generic_wood_router(),
        Some(&steep),
    );
    assert_eq!(sourced(record(&s)).arm, RampArm::ChipTerm);
    assert_eq!(
        s.provenance.ramp_feed_rate,
        Some(ValueProvenance::vendor_lut(SOFTWOOD_6))
    );

    let helix = helix_dressup(2.0, 1.0);
    let s = suggest(&pocket(), &tool, &material, &open_router(), Some(&helix));
    assert_eq!(sourced(record(&s)).arm, RampArm::CutFeed);
    assert!(s.provenance.feed_rate.is_some());
    assert_eq!(s.provenance.ramp_feed_rate, s.provenance.feed_rate);

    let off = no_entry();
    let s = suggest(&pocket(), &tool, &material, &open_router(), Some(&off));
    assert!(s.provenance.plunge_rate.is_some());
    assert_eq!(s.provenance.ramp_feed_rate, s.provenance.plunge_rate);

    // A hand edit after the Suggest: the flush detects it.
    let s = suggest(&pocket(), &tool, &material, &open_router(), Some(&helix));
    let old_op = s.operation.clone();
    let mut new_op = old_op.clone();
    assert!(new_op.as_params_mut().set_ramp_feed_rate(Some(1234.0)));
    let old = s.provenance;
    let mut entry = old.clone();
    entry.detect_manual_edits(&old_op, &new_op, &old);
    assert_eq!(
        entry.ramp_feed_rate.map(|p| p.source),
        Some(ProvenanceSource::Manual)
    );
}
