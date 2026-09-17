//! P1 sentries — per-axis maximum rate (`$110/$111/$112`).
//!
//! `MachineKinematics::max_rate_xyz_mm_min` completes the machine model:
//! the integrator already knew each axis' acceleration but not its
//! maximum rate, so a Z-dominant move was modelled at the X/Y travel
//! rate while the controller clamps it to `$112`.
//!
//! The four sentries here pin:
//!
//! * (a) the field UNSET leaves `compute_cycle_time_breakdown` and
//!   `predicted_feeds_for_toolpath` **bit-identical** to their pre-P1
//!   values on five fixtures, one of which carries rapids;
//! * (b) a pure-Z fed move commanded at 1807 mm/min (the figure the
//!   wanaka front rough emitted) integrates at `$112 = 1000` and reports
//!   `KinematicBinding::RateBound { axis: 2 }`;
//! * (c) a planar XY move with the same field set is untouched by the Z
//!   rate — its peak equals the unset case exactly;
//! * (d) a `$$` round trip carries all three rates, and a
//!   `MachineKinematics` holding them survives serde unchanged.
//!
//! Sentry (e), the wanaka Back Rough 827 s wall-clock re-check, lives in
//! `machine_kinematics_cycle_time_f034.rs` instead: it needs the
//! user-local wanaka project and runs for ~70 s, so it belongs behind
//! the `heavy-tests` feature with the rest of the calibration, not in
//! this dev-loop file.
//!
//! The expected numbers below were captured from the pre-P1 code on
//! 2026-09-07 and are pinned as raw IEEE-754 bit patterns, not
//! tolerances: "byte-identical when unset" is the whole promise.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::{
    KinematicBinding, MachineKinematics, compute_cycle_time_breakdown, move_kinematics,
    predicted_feeds_for_toolpath,
};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 10_000.0;

/// The user's tuned Shapeoko rates, `$110/$111/$112`.
const TUNED_RATES_MM_MIN: [f64; 3] = [10_000.0, 10_000.0, 1_000.0];

/// The tuned Shapeoko kinematics WITHOUT the per-axis rates, spelled out
/// inline rather than taken from `shapeoko_xxl_ricky_tuned()` — that
/// constructor now carries the rates, and the unset arm must keep
/// measuring the unset case.
fn tuned_without_rates() -> MachineKinematics {
    MachineKinematics {
        acceleration_mm_s2: (500.0 + 500.0 + 270.0) / 3.0,
        acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0]),
        junction_deviation_mm: 0.020,
        ..MachineKinematics::default()
    }
}

fn tuned_with_rates() -> MachineKinematics {
    MachineKinematics {
        max_rate_xyz_mm_min: Some(TUNED_RATES_MM_MIN),
        ..tuned_without_rates()
    }
}

// ---- fixtures --------------------------------------------------------

/// 100 mm straight X feed — full trapezoid.
fn fixture_straight() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to(P3::new(100.0, 0.0, 0.0), 3000.0);
    tp
}

/// 20-reversal zigzag — corner-heavy, exercises the junction model.
fn fixture_zigzag() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 1i32..=20 {
        let y = if i % 2 == 0 { 1.0 } else { 0.0 };
        tp.feed_to(P3::new(f64::from(i), y, 0.0), 3000.0);
    }
    tp
}

/// 1 mm move — triangular profile.
fn fixture_triangular() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to(P3::new(1.0, 0.0, 0.0), 3000.0);
    tp
}

/// Pure-Z fed descent at the 1807 mm/min the wanaka front rough emitted.
fn fixture_pure_z() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to(P3::new(0.0, 0.0, -6.0), 1807.0);
    tp
}

/// Mixed 3D path with rapids — the Z-only rapids are where the "rapids
/// obey `$110-112` too" ruling lands.
fn fixture_mixed_with_rapids() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.rapid_to(P3::new(0.0, 0.0, -2.0));
    tp.feed_to_with_intent(P3::new(30.0, 0.0, -2.0), 2400.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(30.0, 20.0, -4.0), 2400.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(0.0, 20.0, -4.0), 2400.0, MoveIntent::ClearingCut);
    tp.rapid_to(P3::new(0.0, 20.0, 10.0));
    tp.rapid_to(P3::new(50.0, 50.0, 10.0));
    tp.rapid_to(P3::new(50.0, 50.0, -1.0));
    tp.feed_to_with_intent(P3::new(50.0, 50.0, -5.0), 1807.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(70.0, 50.0, -5.0), 2400.0, MoveIntent::ClearingCut);
    tp
}

// ---- (a) unset ⇒ bit-identical ---------------------------------------

/// Pre-P1 `CycleTimeBreakdown::total_s` bit patterns, captured
/// 2026-09-07 from `master` before the field existed. Any change to
/// these means the per-axis-rate work reached a machine that has no
/// per-axis rates — the one thing P1 promised it would not do.
const PRE_P1_TOTAL_BITS: [(&str, u64); 5] = [
    ("straight", 0x4000_cccc_cccc_cccd),
    ("zigzag", 0x3ff7_d2b3_fd95_ed53),
    ("triangular", 0x3fb6_e5b7_d166_57e1),
    ("pure_z", 0x3fd3_e3a1_550d_e876),
    ("mixed", 0x4013_b4db_ebf1_5c8c),
];

/// Pre-P1 `predicted_feeds_for_toolpath` bit patterns for the mixed
/// fixture, keyed by source move index. Move 1 and 7 are Z-dominant
/// rapids and move 8 is the 1807 mm/min pure-Z feed — exactly the moves
/// the rate cap moves once the field is set.
const PRE_P1_MIXED_FEED_BITS: [(usize, u64); 9] = [
    (1, 0x40aa_ae85_0e12_4c51),
    (2, 0x40a2_c000_0000_0000),
    (3, 0x40a2_c000_0000_0000),
    (4, 0x40a2_c000_0000_0000),
    (5, 0x40ac_d1cd_f716_2569),
    (6, 0x40c3_8800_0000_0000),
    (7, 0x40a9_8bb9_8a30_728a),
    (8, 0x409c_3c00_0000_0000),
    (9, 0x40a2_c000_0000_0000),
];

fn fixture_by_name(name: &str) -> Toolpath {
    match name {
        "straight" => fixture_straight(),
        "zigzag" => fixture_zigzag(),
        "triangular" => fixture_triangular(),
        "pure_z" => fixture_pure_z(),
        "mixed" => fixture_mixed_with_rapids(),
        other => panic!("unknown fixture {other}"),
    }
}

#[test]
fn unset_rate_field_leaves_the_integrator_bit_identical() {
    let kin = tuned_without_rates();
    assert_eq!(
        kin.max_rate_xyz_mm_min, None,
        "the unset arm must actually be unset"
    );
    for (name, expected_bits) in PRE_P1_TOTAL_BITS {
        let tp = fixture_by_name(name);
        let breakdown = compute_cycle_time_breakdown(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
        assert_eq!(
            breakdown.total_s.to_bits(),
            expected_bits,
            "fixture {name}: total_s must stay bit-identical to the pre-P1 value \
             (got {got:.17e} = {got_bits:#x}, expected {expected_bits:#x})",
            got = breakdown.total_s,
            got_bits = breakdown.total_s.to_bits()
        );
    }
}

#[test]
fn unset_rate_field_leaves_predicted_feeds_bit_identical() {
    let kin = tuned_without_rates();
    let tp = fixture_mixed_with_rapids();
    let feeds = predicted_feeds_for_toolpath(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    assert_eq!(feeds.len(), PRE_P1_MIXED_FEED_BITS.len());
    for (index, expected_bits) in PRE_P1_MIXED_FEED_BITS {
        let got = feeds.get(&index).copied().expect("move present");
        assert_eq!(
            got.to_bits(),
            expected_bits,
            "move {index}: predicted feed must stay bit-identical to the pre-P1 value \
             (got {got:.17e} = {:#x})",
            got.to_bits()
        );
    }
}

// ---- (b) a pure-Z move is capped at $112 -----------------------------

#[test]
fn pure_z_move_is_rate_bound_at_the_z_axis_rate() {
    let kin = tuned_with_rates();
    let dir = [0.0, 0.0, -1.0];
    // 6 mm is long enough to reach 1000 mm/min (16.67 mm/s) at the
    // Z axis' 270 mm/s²: the ramp needs only 0.51 mm.
    let mk = move_kinematics(6.0, &dir, 0.0, 0.0, 1807.0, &kin, MAX_FEED_MM_MIN);
    assert!(
        (mk.peak_mm_min - 1000.0).abs() < 1e-9,
        "a pure-Z move commanded at 1807 mm/min must peak at $112 = 1000, got {}",
        mk.peak_mm_min
    );
    assert_eq!(
        mk.binding,
        KinematicBinding::RateBound { axis: 2 },
        "the Z axis is what capped it"
    );

    // The same direction-aware ceiling, read directly.
    let rate = kin
        .effective_max_rate_mm_min(&dir)
        .expect("per-axis rates are set");
    assert!((rate - 1000.0).abs() < 1e-9, "got {rate}");

    // And the integrator agrees: the fixture takes strictly longer than
    // it did with no rate cap.
    let tp = fixture_pure_z();
    let capped = compute_cycle_time_breakdown(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let uncapped = compute_cycle_time_breakdown(
        &tp,
        &tuned_without_rates(),
        MAX_FEED_MM_MIN,
        RAPID_FEED_MM_MIN,
    );
    assert!(
        capped.total_s > uncapped.total_s,
        "the Z rate cap must lengthen a pure-Z descent: {} vs {}",
        capped.total_s,
        uncapped.total_s
    );
    let feeds = predicted_feeds_for_toolpath(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let peak = feeds.get(&1).copied().expect("the descent");
    assert!(
        (peak - 1000.0).abs() < 1e-9,
        "the predicted-feed map must report the capped 1000 mm/min, got {peak}"
    );
}

/// The ruling of deliverable 4(a): GRBL clamps `G0` by `$110-112` too,
/// so a Z-only rapid is limited by `$112`, not by the X/Y travel rate.
#[test]
fn a_z_only_rapid_obeys_the_z_rate_too() {
    let kin = tuned_with_rates();
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.rapid_to(P3::new(0.0, 0.0, -2.0));
    let feeds = predicted_feeds_for_toolpath(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let peak = feeds.get(&1).copied().expect("the Z rapid");
    assert!(
        peak <= 1000.0 + 1e-9,
        "a Z-only rapid must not exceed $112 = 1000 mm/min, got {peak}"
    );
    // Without the rates it ran far above the Z axis' real ceiling.
    let loose = predicted_feeds_for_toolpath(
        &tp,
        &tuned_without_rates(),
        MAX_FEED_MM_MIN,
        RAPID_FEED_MM_MIN,
    );
    let loose_peak = loose.get(&1).copied().expect("the Z rapid");
    assert!(
        loose_peak > 1000.0,
        "the pre-P1 model really did run this rapid above $112 ({loose_peak})"
    );
}

// ---- (c) a planar move is untouched ----------------------------------

#[test]
fn a_planar_xy_move_is_untouched_by_the_z_rate() {
    let with_rates = tuned_with_rates();
    let without = tuned_without_rates();
    let dir = [1.0, 0.0, 0.0];

    let capped = move_kinematics(100.0, &dir, 0.0, 0.0, 3000.0, &with_rates, MAX_FEED_MM_MIN);
    let loose = move_kinematics(100.0, &dir, 0.0, 0.0, 3000.0, &without, MAX_FEED_MM_MIN);
    assert_eq!(
        capped.peak_mm_min.to_bits(),
        loose.peak_mm_min.to_bits(),
        "the Z rate must not touch a planar move: {} vs {}",
        capped.peak_mm_min,
        loose.peak_mm_min
    );
    assert_eq!(capped.binding, KinematicBinding::FeedBound);
    assert_eq!(loose.binding, KinematicBinding::FeedBound);

    // A 45° XY diagonal too — each axis carries only its own component,
    // so the X/Y ceiling relaxes rather than binds.
    let diag = [0.5_f64.sqrt(), 0.5_f64.sqrt(), 0.0];
    let rate = with_rates
        .effective_max_rate_mm_min(&diag)
        .expect("rates set");
    assert!(
        (rate - 10_000.0 / 0.5_f64.sqrt()).abs() < 1e-6,
        "diagonal ceiling should be 10000/sin45 ≈ 14142, got {rate}"
    );

    // Whole-toolpath check: the two planar fixtures integrate to the
    // same bits with the rates set as without them.
    for name in ["straight", "zigzag", "triangular"] {
        let tp = fixture_by_name(name);
        let a = compute_cycle_time_breakdown(&tp, &with_rates, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
        let b = compute_cycle_time_breakdown(&tp, &without, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
        assert_eq!(
            a.total_s.to_bits(),
            b.total_s.to_bits(),
            "planar fixture {name} must be unmoved by the Z rate: {} vs {}",
            a.total_s,
            b.total_s
        );
    }
}

/// The binding vocabulary, on the three cases that are not `RateBound`.
#[test]
fn binding_names_what_limited_the_peak() {
    let kin = tuned_with_rates();
    let x = [1.0, 0.0, 0.0];

    // Long planar move from rest to rest: reaches the command.
    let feed_bound = move_kinematics(100.0, &x, 0.0, 0.0, 3000.0, &kin, MAX_FEED_MM_MIN);
    assert_eq!(feed_bound.binding, KinematicBinding::FeedBound);

    // 1 mm from rest to rest at 3000 mm/min: rose above both (zero)
    // junctions but ran out of length.
    let accel_bound = move_kinematics(1.0, &x, 0.0, 0.0, 3000.0, &kin, MAX_FEED_MM_MIN);
    assert_eq!(accel_bound.binding, KinematicBinding::AccelBound);
    assert!(accel_bound.peak_mm_min < 3000.0);

    // A short move entered fast and left at rest: it never rises above
    // its entry speed, so the junction is what set the peak.
    let junction_bound = move_kinematics(0.05, &x, 2500.0, 0.0, 3000.0, &kin, MAX_FEED_MM_MIN);
    assert_eq!(junction_bound.binding, KinematicBinding::JunctionBound);
}

// ---- (d) $$ round trip + serde ---------------------------------------

#[test]
fn grbl_round_trip_carries_all_three_rates() {
    let dump = "\
$11=0.020 (junction deviation, mm)\n\
$110=10000.000\n$111=10000.000\n$112=1000.000\n\
$120=500.000\n$121=500.000\n$122=270.000\n";
    let imported = MachineKinematics::from_grbl_settings(dump);
    assert_eq!(
        imported.max_rate_xyz_mm_min,
        Some(TUNED_RATES_MM_MIN),
        "the $$ parser must carry $110/$111/$112 as a triple"
    );
    // The two pre-existing scalar slots keep their meaning.
    assert_eq!(imported.max_feed_mm_min, Some(10_000.0));
    assert_eq!(imported.max_z_feed_mm_min, Some(1_000.0));

    // A partial rate set yields no triple — a direction-aware ceiling
    // cannot be built from two axes.
    let partial = MachineKinematics::from_grbl_settings("$110=10000\n$111=10000\n");
    assert_eq!(partial.max_rate_xyz_mm_min, None);
    assert_eq!(partial.max_feed_mm_min, Some(10_000.0));

    // The tuned constructor and the parser must not drift apart.
    let preset = MachineKinematics::shapeoko_xxl_ricky_tuned();
    assert_eq!(preset.max_rate_xyz_mm_min, imported.max_rate_xyz_mm_min);
}

#[test]
fn kinematics_with_rates_survive_serde_unchanged() {
    let kin = tuned_with_rates();
    let json = serde_json::to_string(&kin).expect("serialize");
    let back: MachineKinematics = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(kin, back);
    assert_eq!(back.max_rate_xyz_mm_min, Some(TUNED_RATES_MM_MIN));

    // A project file written before P1 carries no key at all and must
    // deserialize to `None`, not to a default triple.
    let legacy = r#"{"acceleration_mm_s2":350.0,"acceleration_xyz_mm_s2":[500.0,500.0,270.0],
        "junction_deviation_mm":0.02,"jerk_mm_s3":null,"max_junction_velocity_mm_min":null}"#;
    let old: MachineKinematics = serde_json::from_str(legacy).expect("legacy deserialize");
    assert_eq!(old.max_rate_xyz_mm_min, None);
}

// ---- (f) EDG-07 — one digest-and-junction-walk pass -------------------
//
// `compute_cycle_time_breakdown` and `predicted_feeds_for_toolpath` built
// two copies of the same `MoveDigest` and walked the same junction
// integrator twice. EDG-07 folds them onto one `digest_moves` +
// `walk_junctions` pass. The refactor must not move a single bit, and
// sentries (a) and (b) above cannot prove that on their own: they pin
// `total_s` only, with the per-axis rates UNSET and no jerk limit.
//
// This case pins every `CycleTimeBreakdown` field AND every predicted
// feed, with the rates set, a jerk limit set and a junction-velocity cap
// set, on a fixture that reaches every branch of the shared pass: a
// rapid, an arc, a zero-length move the digest drops, a triangular move,
// and one feed move per intent bucket the breakdown names. Bit patterns,
// not tolerances.

fn tuned_with_rates_and_jerk() -> MachineKinematics {
    MachineKinematics {
        jerk_mm_s3: Some(30_000.0),
        max_junction_velocity_mm_min: Some(4_000.0),
        ..tuned_with_rates()
    }
}

/// Every branch of the shared pass, in one toolpath.
fn fixture_every_branch() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    // Zero length: the digest drops it, so it must not shift an index.
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.rapid_to(P3::new(5.0, 5.0, 10.0));
    tp.feed_to_with_intent(P3::new(5.0, 5.0, -2.0), 600.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(6.0, 5.0, -2.5), 900.0, MoveIntent::EntryRamp);
    tp.feed_to_with_intent(P3::new(8.0, 5.0, -3.0), 1200.0, MoveIntent::EntryHelix);
    tp.feed_to_with_intent(P3::new(10.0, 5.0, -3.0), 1500.0, MoveIntent::LeadIn);
    tp.feed_to_with_intent(P3::new(40.0, 5.0, -3.0), 2400.0, MoveIntent::ClearingCut);
    tp.arc_cw_to_with_intent(
        P3::new(45.0, 10.0, -3.0),
        0.0,
        5.0,
        2400.0,
        MoveIntent::FinishingCut,
    );
    // 0.2 mm: too short to reach the commanded feed — triangular.
    tp.feed_to_with_intent(P3::new(45.2, 10.0, -3.0), 2400.0, MoveIntent::FinishingCut);
    tp.feed_to_with_intent(P3::new(45.2, 10.0, -1.0), 1800.0, MoveIntent::LeadOut);
    tp.feed_to_with_intent(P3::new(45.2, 10.0, 10.0), 1800.0, MoveIntent::Retract);
    tp.feed_to_with_intent(P3::new(60.0, 30.0, 10.0), 3000.0, MoveIntent::Linking);
    // Untagged: a legacy generator's move lands in `unknown_s`.
    tp.feed_to(P3::new(60.0, 30.0, -4.0), 1807.0);
    tp.feed_to_with_intent(P3::new(62.0, 30.0, -4.0), 2400.0, MoveIntent::Drilling);
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp
}

/// Captured 2026-09-17 from the two-pass code, before EDG-07 merged them.
const EDG07_BREAKDOWN_BITS: [(&str, u64); 7] = [
    ("total_s", 0x4019_c56c_e815_d179),
    ("rapid_s", 0x3ff4_5c5a_7580_5555),
    ("cutting_s", 0x3ff4_5bb7_1a62_e3d6),
    ("entry_s", 0x3ff8_ecdd_2f9e_d6a9),
    ("linking_s", 0x3fe8_32e2_ac8d_e28d),
    ("retract_s", 0x3fe6_039a_ae4a_5da5),
    ("unknown_s", 0x3fec_ab0c_66d2_2bed),
];

/// Captured 2026-09-17 from the two-pass code, before EDG-07 merged them.
const EDG07_FEED_BITS: [(usize, u64); 14] = [
    (2, 0x40b0_92a4_0412_304c),
    (3, 0x4082_c000_0000_0000),
    (4, 0x408c_2000_0000_0000),
    (5, 0x4092_c000_0000_0000),
    (6, 0x4097_7000_0000_0000),
    (7, 0x40a2_c000_0000_0000),
    (8, 0x40a2_c000_0000_0000),
    (9, 0x4088_3ebf_e283_ac58),
    (10, 0x408f_4000_0000_0001),
    (11, 0x408f_4000_0000_0001),
    (12, 0x40a7_7000_0000_0000),
    (13, 0x408f_4000_0000_0001),
    (14, 0x409d_be29_6599_6b0c),
    (15, 0x40b3_9c5d_def1_5bfc),
];

#[test]
fn one_shared_pass_leaves_both_integrators_bit_identical_edg07() {
    let kin = tuned_with_rates_and_jerk();
    let tp = fixture_every_branch();

    let breakdown = compute_cycle_time_breakdown(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let got: Vec<(&str, u64)> = vec![
        ("total_s", breakdown.total_s.to_bits()),
        ("rapid_s", breakdown.rapid_s.to_bits()),
        ("cutting_s", breakdown.cutting_s.to_bits()),
        ("entry_s", breakdown.entry_s.to_bits()),
        ("linking_s", breakdown.linking_s.to_bits()),
        ("retract_s", breakdown.retract_s.to_bits()),
        ("unknown_s", breakdown.unknown_s.to_bits()),
    ];
    assert_eq!(
        got,
        EDG07_BREAKDOWN_BITS.to_vec(),
        "every CycleTimeBreakdown field must stay bit-identical across the \
         EDG-07 extraction"
    );

    let feeds = predicted_feeds_for_toolpath(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    let got_feeds: Vec<(usize, u64)> = feeds
        .iter()
        .map(|(index, feed)| (*index, feed.to_bits()))
        .collect();
    assert_eq!(
        got_feeds,
        EDG07_FEED_BITS.to_vec(),
        "every predicted feed, and the source-move index it is keyed by, \
         must stay bit-identical across the EDG-07 extraction"
    );
}
