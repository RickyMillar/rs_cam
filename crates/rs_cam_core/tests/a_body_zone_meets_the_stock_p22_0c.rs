//! P22-0c-4 (`planning/feeds_contract_2026-11-03/phases/P22-0c-4.md`):
//! the contact kernel tests the shank, the holder and the shoe of the
//! mounted tool on the live stock.
//!
//! - A shank wider than the cutter meets the walls of a slot deeper than
//!   the flute (a fed `Shank` contact). A shank no wider than the cutter
//!   gives none in the same slot.
//! - A holder meets the unroughed stock around a deep pocket (a fed
//!   `Holder` contact).
//! - A plain end mill (shank no wider than the cutter) in a cleared pocket
//!   deeper than its flute gives no new finding. The same pocket with a
//!   shank 1 mm wider gives `Shank` contacts.
//! - A rapid whose shank passes through stock gives a rapid contact and a
//!   struck rapid (`project.rapid_collision`).
//!
//! Red before P22-0c-4: the kernel gave no probe for these zones, so each
//! finding arm had no contact.
//!
//! `cargo test -p rs_cam_core -q --test a_body_zone_meets_the_stock_p22_0c`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::head::HeadBand;
use rs_cam_core::stock::collision::ContactMotion;
use rs_cam_core::stock::contact::{ContactRule, WalkChecks, WalkFindings};
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::tool::body::ContactKind;
use rs_cam_core::toolpath::Toolpath;

const CELL_MM: f64 = 0.5;

/// A centre-cutting end mill of `diameter`, flute `flute`, shank `shank`
/// and stickout `stickout` (mm).
fn end_mill(diameter: f64, flute: f64, shank: f64, stickout: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = diameter;
    tool.set_cutting_length(ToolFact::Stated(flute));
    tool.set_shank_diameter(ToolFact::Stated(shank));
    tool.set_stickout(ToolFact::Stated(stickout));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

/// One step of a hand-made toolpath.
#[derive(Clone, Copy)]
enum Step {
    Feed(P3),
    Rapid(P3),
}

/// A slot along X from `-half_len` to `half_len` at Y 0, cut in zigzag
/// layers from `first_z` down to `last_z` by `step` (mm), from a rapid
/// over the start.
fn slot(half_len: f64, first_z: f64, last_z: f64, step: f64, clear_z: f64) -> Vec<Step> {
    let mut steps = vec![
        Step::Rapid(P3::new(-half_len, 0.0, clear_z)),
        Step::Rapid(P3::new(-half_len, 0.0, first_z + step)),
    ];
    let mut z = first_z;
    let mut at_start = true;
    while z >= last_z - 1e-9 {
        let x_from = if at_start { -half_len } else { half_len };
        steps.push(Step::Feed(P3::new(x_from, 0.0, z)));
        steps.push(Step::Feed(P3::new(-x_from, 0.0, z)));
        at_start = !at_start;
        z -= step;
    }
    steps
}

/// The walk findings of `steps` over a fresh block `[-half, half]^2 x [0,
/// top]`, with the tool mounted with `bands` at its holder face.
fn walk(
    tool: &ToolConfig,
    bands: Vec<HeadBand>,
    half: f64,
    top: f64,
    steps: &[Step],
) -> WalkFindings {
    walk_with(tool, bands, half, top, steps, None)
}

/// [`walk`] under `rule` (`None`: the shipped rule).
fn walk_with(
    tool: &ToolConfig,
    bands: Vec<HeadBand>,
    half: f64,
    top: f64,
    steps: &[Step],
    rule: Option<ContactRule>,
) -> WalkFindings {
    let def = rs_cam_core::compute::cutter::build_cutter(tool).expect("a valid tool");
    let lut = RadialProfileLUT::from_cutter(&def, LUT_SAMPLES);
    let mut stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(-half, -half, 0.0),
            max: P3::new(half, half, top),
        },
        CELL_MM,
    );
    let mut tp = Toolpath::new();
    for step in steps {
        match *step {
            Step::Feed(p) => tp.feed_to(p, 1000.0),
            Step::Rapid(p) => tp.rapid_to(p),
        }
    }
    let mounted = def.body().mounted(bands);
    let mut check = match rule {
        Some(rule) => WalkChecks::mounted_with_rule(&def, &mounted, rule),
        None => WalkChecks::mounted(&def, &mounted),
    };
    let never = || false;
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            &tp,
            &lut,
            &def,
            def.radius(),
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            18_000,
            2,
            5000.0,
            0.25,
            None,
            &[],
            &[],
            true,
            &never,
            Some(&mut check),
        )
        .expect("the walk completes");
    check.into_findings()
}

fn zone_contacts(found: &WalkFindings, zone: ContactKind, motion: ContactMotion) -> usize {
    found
        .body
        .iter()
        .filter(|c| c.zone == zone && c.motion == motion)
        .count()
}

/// A 3 mm cutter with a 6 mm shank and a 6 mm flute cuts a slot 12 mm
/// deep in 1 mm layers. Below 6 mm the shank meets the slot walls the
/// cutter left; above it the shank is in air. A 3 mm shank gives none.
#[test]
fn a_shank_wider_than_the_cutter_meets_standing_wood() {
    let top = 20.0;
    let steps = slot(8.0, top - 1.0, top - 12.0, 1.0, top + 5.0);
    let found = walk(
        &end_mill(3.0, 6.0, 6.0, 30.0),
        Vec::new(),
        20.0,
        top,
        &steps,
    );
    let shank = zone_contacts(&found, ContactKind::Shank, ContactMotion::Fed);
    assert!(
        shank > 0,
        "the shank meets the slot walls: {:?}",
        found.body
    );
    for c in &found.body {
        assert!(
            c.position.z < top - 6.0 + 1e-6,
            "a contact only below the flute depth: {c:?}"
        );
        assert!(c.material_over_tip_mm > c.zone_bottom_mm, "{c:?}");
        assert_eq!(c.name, None, "a tool zone has no part name");
    }
    assert!(
        found.flute.is_empty(),
        "the flutes cut the slot floor 1 mm at a time: {:?}",
        found.flute
    );

    let plain = walk(
        &end_mill(3.0, 6.0, 3.0, 30.0),
        Vec::new(),
        20.0,
        top,
        &steps,
    );
    assert!(
        plain.body.is_empty(),
        "a shank no wider than the cutter gives no contact: {:?}",
        plain.body
    );
}

/// A 6 mm end mill (flute 25, stickout 30) in a 30 mm holder clears a
/// pocket 34 mm deep in 2 mm layers. Below 30 mm the holder face meets the
/// unroughed stock around the pocket.
#[test]
fn a_holder_meets_the_unroughed_stock_in_a_deep_pocket() {
    let top = 40.0;
    let steps = slot(5.0, top - 2.0, top - 34.0, 2.0, top + 5.0);
    let tool = end_mill(6.0, 25.0, 6.0, 30.0);
    let found = walk(
        &tool,
        vec![HeadBand::holder_at_face(30.0, 30.0)],
        30.0,
        top,
        &steps,
    );
    let holder = zone_contacts(&found, ContactKind::Holder, ContactMotion::Fed);
    assert!(holder > 0, "the holder meets the stock: {:?}", found.body);
    for c in found.body.iter().filter(|c| c.zone == ContactKind::Holder) {
        assert!(c.position.z < top - 30.0 + 1e-6, "{c:?}");
        assert_eq!(c.name.as_deref(), Some("Holder"));
        assert!((c.zone_bottom_mm - 30.0).abs() < 1e-9, "{c:?}");
    }
    assert_eq!(
        zone_contacts(&found, ContactKind::Shank, ContactMotion::Fed),
        0,
        "the shank is no wider than the cutter (shadow rule)"
    );
    assert!(found.flute.is_empty(), "{:?}", found.flute);

    // The same pocket with no holder band: no contact at all.
    let bare = walk(&tool, Vec::new(), 30.0, top, &steps);
    assert!(bare.body.is_empty(), "{:?}", bare.body);
}

/// A plain end mill (6 mm, shank 6 mm, flute 25, stickout 45) clears a
/// pocket 34 mm deep, deeper than its flute. No new finding: no body
/// contact, no flute contact, no struck rapid. The same pocket with a
/// 7 mm shank gives `Shank` contacts (the arm can fail).
#[test]
fn a_plain_end_mill_in_a_cleared_pocket_gives_no_new_finding() {
    let top = 40.0;
    let steps = slot(5.0, top - 2.0, top - 34.0, 2.0, top + 5.0);
    let found = walk(
        &end_mill(6.0, 25.0, 6.0, 45.0),
        Vec::new(),
        30.0,
        top,
        &steps,
    );
    assert!(found.body.is_empty(), "{:?}", found.body);
    assert!(found.flute.is_empty(), "{:?}", found.flute);
    assert!(found.rapids.is_empty(), "{:?}", found.rapids);

    let wide = walk(
        &end_mill(6.0, 25.0, 7.0, 45.0),
        Vec::new(),
        30.0,
        top,
        &steps,
    );
    assert!(
        zone_contacts(&wide, ContactKind::Shank, ContactMotion::Fed) > 0,
        "a shank 1 mm wider than the cutter meets the pocket walls"
    );
}

/// After the 12 mm slot of the first test, a rapid goes down into the slot
/// and along it, 0.5 mm over its floor. The cutter envelope is in the cut
/// slot; the 6 mm shank meets the walls above the flute.
#[test]
fn a_rapid_whose_shank_passes_through_stock_gives_a_finding() {
    let top = 20.0;
    let floor = top - 10.0;
    let mut steps = slot(8.0, top - 1.0, floor, 1.0, top + 5.0);
    let last = match steps.last() {
        Some(Step::Feed(p) | Step::Rapid(p)) => *p,
        None => unreachable!("the slot has steps"),
    };
    steps.push(Step::Rapid(P3::new(last.x, 0.0, top + 5.0)));
    steps.push(Step::Rapid(P3::new(-6.0, 0.0, top + 5.0)));
    steps.push(Step::Rapid(P3::new(-6.0, 0.0, floor + 0.5)));
    steps.push(Step::Rapid(P3::new(6.0, 0.0, floor + 0.5)));
    let rapid_moves = steps.len() - 2..steps.len();

    let found = walk(
        &end_mill(3.0, 6.0, 6.0, 30.0),
        Vec::new(),
        20.0,
        top,
        &steps,
    );
    let rapid_shank: Vec<_> = found
        .body
        .iter()
        .filter(|c| c.zone == ContactKind::Shank && c.motion == ContactMotion::Rapid)
        .collect();
    assert!(
        !rapid_shank.is_empty(),
        "the shank of the rapid meets the slot walls: {:?}",
        found.body
    );
    for c in &rapid_shank {
        assert!(rapid_moves.contains(&c.move_index), "{c:?}");
        assert!(
            found.rapids.iter().any(|r| r.move_index == c.move_index),
            "a rapid contact is a struck rapid: {:?}",
            found.rapids
        );
    }
    // The rapid straight up out of the slot is exempt.
    let up = steps.len() - 4;
    assert!(
        !found.body.iter().any(|c| c.move_index == up),
        "{:?}",
        found.body
    );
}

/// Ruling Q2, the case the in-move rule exists for. A pocket is cleared
/// to Z10 for X >= -2 in a 20 mm block; a 6 mm cutter with a 6 mm flute
/// then ramps in one move from (-8, 0, 15) over uncut wood down to
/// (20, 0, 10.5) over the pocket. The flutes cut the uncut wood behind the
/// tip on the way down (each column came under the cutter while the tip
/// was at most 6 mm under its top). The pre-move rule reads that wood as
/// standing and reports the flute top; the in-move rule reads the cut and
/// reports nothing. The shipped rule is the pre-move rule (the fixture
/// count found no case; `stock::contact::SHIPPED_RULE`).
#[test]
fn the_in_move_rule_removes_a_ramps_own_cut_q2() {
    let top = 20.0;
    let mut steps = vec![Step::Rapid(P3::new(1.0, -6.0, top + 5.0))];
    let mut z = top - 2.0;
    while z >= 10.0 - 1e-9 {
        let mut y = -6.0;
        let mut forward = true;
        while y <= 6.0 + 1e-9 {
            let (a, b) = if forward { (1.0, 25.0) } else { (25.0, 1.0) };
            steps.push(Step::Feed(P3::new(a, y, z)));
            steps.push(Step::Feed(P3::new(b, y, z)));
            forward = !forward;
            y += 2.0;
        }
        steps.push(Step::Rapid(P3::new(1.0, -6.0, z)));
        z -= 2.0;
    }
    steps.push(Step::Rapid(P3::new(1.0, -6.0, top + 5.0)));
    steps.push(Step::Rapid(P3::new(-8.0, 0.0, top + 5.0)));
    steps.push(Step::Feed(P3::new(-8.0, 0.0, 15.0)));
    steps.push(Step::Feed(P3::new(20.0, 0.0, 10.5)));
    // `steps` holds the first rapid as move 0, so its last index is the
    // ramp's move index.
    let ramp = steps.len() - 1;

    let tool = end_mill(6.0, 6.0, 6.0, 30.0);
    let pre = walk_with(
        &tool,
        Vec::new(),
        30.0,
        top,
        &steps,
        Some(ContactRule::PreMove),
    );
    assert!(
        pre.flute.iter().any(|f| f.move_index == ramp),
        "the pre-move rule reads the cut wood as standing: {:?}",
        pre.flute
    );
    let inm = walk_with(
        &tool,
        Vec::new(),
        30.0,
        top,
        &steps,
        Some(ContactRule::InMove),
    );
    assert!(
        inm.flute.is_empty(),
        "the in-move rule reads the ramp's own cut: {:?}",
        inm.flute
    );
    // The other moves give the same answer under both rules.
    let others = |f: &WalkFindings| {
        f.flute
            .iter()
            .filter(|x| x.move_index != ramp)
            .map(|x| x.move_index)
            .collect::<Vec<_>>()
    };
    assert_eq!(others(&pre), others(&inm));
    assert!(pre.body.is_empty() && inm.body.is_empty());
}
