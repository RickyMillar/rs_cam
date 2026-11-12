//! Feeds contract P24a (`planning/feeds_contract_2026-11-03/phases/P24a.md`;
//! operator direction 2026-11-10: "build out the support for our engine for
//! bits like the 18 mm ... the cutters just point straight out, less of a
//! slice and more of a shear").
//!
//! A tool states that it is a straight-flute bit with one fact,
//! `BitCutDirection::Straight`. The fact gives it helix 0
//! (`ToolConfig::hold_flute_form`) and the straight vendor series
//! (`feeds::series::ToolSeries::Straight`). A straight row serves only a
//! straight tool (lead ruling 2026-11-10): the exclusive series of
//! `feeds::series::serves_only_its_own_series`.
//!
//! The arms:
//!
//! - (a) the tool side: the setter `cut_direction = "straight"` writes
//!   helix 0; the session doors `AddTool` and `ReplaceTool` hold the pair;
//!   a record built by hand with a straight flute and a helix is outside
//!   the domain; the cutter of a straight tool has helix 0;
//! - (b) an 18 mm 2-flute straight tool in white oak reads the Onsrud
//!   48-000 rows: the band holds the form A interpolation of the 5/8 in
//!   and 3/4 in rows, and the lowest member ships; the chip is the log-log
//!   interpolation of the two printed band middles;
//! - (c) a generic (up-cut) tool never reads a straight row: at 16, 18,
//!   19.05, 20 and 25.4 mm in hardwood it keeps the Amana Spektra 12.7 mm
//!   row, and no shipped row or band member of any size is straight;
//! - (d) every loaded row whose subfamily names a straight flute, and the
//!   Freud straight table, are of the straight series;
//! - (e) the wanaka idx 1 cut (ap 10 mm, ae `c D` = 5.9913 mm): the peak
//!   engaged-edge force and the tip deflection of the straight edge are at
//!   or above the helix 30 values (the whole depth meets the wood at one
//!   angle). `--nocapture` prints the four numbers.
//!
//! Red: drop the `row_admits_query` filter from
//! `vendor_lookup::passes_must_match` (arm c).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; arm (e) prints its probe values"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::compute::tool_config::{
    BitCutDirection, ToolConfig, ToolFact, ToolId, ToolParamKey, ToolType,
};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::extrapolation::SizeBasis;
use rs_cam_core::feeds::predict::tip_deflection_from_engagement;
use rs_cam_core::feeds::series::{ToolSeries, row_series};
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, suggest_for_operation,
};
use rs_cam_core::feeds::vendor_lut::VendorObservation;
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsResult, SpindleStrategy};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{AddToolArgs, Command, ProjectSession, ReplaceToolArgs};
use rs_cam_core::tool::MillingCutter;

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// A 2-flute flat end mill of `diameter` with the flute form `flute`.
fn flat_2f(diameter: f64, flute: BitCutDirection) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(20.0));
    t.set_stickout(ToolFact::Stated(25.0));
    t.cut_direction = flute;
    let _ = t.hold_flute_form();
    t
}

fn run(op: &OperationConfig, tool: &ToolConfig) -> FeedsResult {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool,
        machine: &MachineProfile::default(),
        material: &white_oak(),
        lut: &EMBEDDED_LUT,
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
    .feeds_result
}

fn obs(id: &str) -> &'static VendorObservation {
    EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .unwrap_or_else(|| panic!("the LUT holds {id}"))
}

fn is_straight(id: &str) -> bool {
    row_series(obs(id)) == Some(ToolSeries::Straight)
}

fn band_mid(id: &str) -> f64 {
    let o = obs(id);
    0.5 * (o.chipload_min_mm_tooth.expect("a printed minimum")
        + o.chipload_max_mm_tooth.expect("a printed maximum"))
}

/// (a) The tool side: one fact, and helix 0 with it.
#[test]
fn a_straight_tool_has_helix_zero_at_every_door_p24a() {
    // The setter.
    let mut tool = flat_2f(18.0, BitCutDirection::UpCut);
    assert!(tool.helix_deg > 0.0, "non-vacuity: the default helix");
    ToolParamKey::CutDirection
        .write(&mut tool, &serde_json::json!("straight"))
        .expect("the token is read");
    assert_eq!(tool.cut_direction, BitCutDirection::Straight);
    assert_eq!(tool.helix_deg, 0.0);
    assert_eq!(BitCutDirection::Straight.token(), "straight");
    assert_eq!(
        ToolSeries::for_cut_direction(tool.cut_direction),
        Some(ToolSeries::Straight)
    );

    // A record built by hand with the pair apart is outside the domain.
    let mut apart = flat_2f(18.0, BitCutDirection::UpCut);
    apart.cut_direction = BitCutDirection::Straight;
    assert!(apart.helix_deg > 0.0);
    let refusal = apart.validate().expect_err("a straight flute has helix 0");
    assert!(refusal.to_string().contains("helix_deg"), "{refusal}");
    assert!(
        refusal
            .to_string()
            .contains("a straight tool has helix 0; change the flute form first"),
        "the refusal names the flute form: {refusal}"
    );

    // The session doors hold the pair: no refusal, helix 0.
    let mut session = ProjectSession::new_empty();
    let added = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(apart.clone()),
        }))
        .expect("AddTool takes a straight tool")
        .created
        .expect("the tool index");
    assert_eq!(session.tools()[added].helix_deg, 0.0);
    let id = session.tools()[added].id;
    let mut replaced = apart;
    replaced.id = id;
    let _ = session
        .apply(Command::ReplaceTool(ReplaceToolArgs {
            tool_id: id,
            config: Box::new(replaced),
        }))
        .expect("ReplaceTool takes a straight tool");
    assert_eq!(session.tools()[added].helix_deg, 0.0);

    // The cutter.
    let cutter = build_cutter(&tool).expect("the tool builds a cutter");
    assert_eq!(cutter.helix_deg(), 0.0);
}

/// (b) A straight tool reads the straight rows of its material and size.
#[test]
fn a_straight_tool_reads_the_straight_rows_p24a() {
    let tool = flat_2f(18.0, BitCutDirection::Straight);
    let r = run(&fixture_op(OperationType::Pocket), &tool);
    let shipped = r.vendor_source.as_deref().expect("a vendor row answers");
    assert!(
        is_straight(shipped),
        "the shipped row {shipped} is straight"
    );
    assert!(
        shipped.starts_with("onsrud-hardwood-48-000-"),
        "the Onsrud 48-000 hard-wood row ships: {shipped}"
    );
    let band = r.vendor_band().expect("a band");
    assert!(
        !band.members.is_empty() && band.members.iter().all(|m| is_straight(&m.row.0)),
        "every member is a straight row: {:?}",
        band.members
    );
    assert!(
        band.members
            .iter()
            .all(|m| matches!(m.size_basis, SizeBasis::Claim(_))),
        "18 mm is between two printed sizes: each member is a size claim"
    );
    // The chip: the log-log interpolation of the band middles of the
    // 5/8 in and 3/4 in rows of the Onsrud hard-wood sheet at 18 mm.
    let (d_lo, d_hi) = (15.875_f64, 19.05_f64);
    let (lo, hi) = (
        band_mid("onsrud-hardwood-48-000-5_8-straight"),
        band_mid("onsrud-hardwood-48-000-3_4-straight"),
    );
    let t = (18.0_f64 / d_lo).ln() / (d_hi / d_lo).ln();
    let want = (lo.ln() + t * (hi / lo).ln()).exp();
    assert!(
        (r.chip_load_mm - want).abs() < 1e-9,
        "chip {} vs the interpolation {want}",
        r.chip_load_mm
    );
    // Above the spiral row that a generic 18 mm tool reads.
    let generic = run(
        &fixture_op(OperationType::Pocket),
        &flat_2f(18.0, BitCutDirection::UpCut),
    );
    assert!(r.chip_load_mm > generic.chip_load_mm);
}

/// (c) A generic tool never reads a straight row.
#[test]
fn a_generic_tool_never_reads_a_straight_row_p24a() {
    // The sizes where a straight row is printed nearer than any standard
    // row: the generic tool keeps the Amana Spektra 12.7 mm row.
    for d in [16.0, 18.0, 19.05, 20.0, 25.4] {
        let r = run(
            &fixture_op(OperationType::Pocket),
            &flat_2f(d, BitCutDirection::UpCut),
        );
        assert_eq!(
            r.vendor_source.as_deref(),
            Some("amana-flat-hardwood-pocket-12700-2f-spektra"),
            "{d} mm"
        );
    }
    // Every size and op: no shipped row and no band member is straight,
    // for each flute form other than the straight one.
    let mut checked = 0;
    for flute in [
        BitCutDirection::UpCut,
        BitCutDirection::DownCut,
        BitCutDirection::Compression,
    ] {
        for d in [3.175, 6.0, 6.35, 9.525, 10.0, 12.7, 16.0, 18.0, 19.05, 25.4] {
            for op_type in [
                OperationType::Pocket,
                OperationType::Adaptive3d,
                OperationType::Adaptive,
                OperationType::Profile,
                OperationType::Face,
            ] {
                let r = run(&fixture_op(op_type), &flat_2f(d, flute));
                if let Some(id) = r.vendor_source.as_deref() {
                    assert!(!is_straight(id), "{flute:?} {d} mm {op_type:?} ships {id}");
                }
                if let Some(band) = r.vendor_band() {
                    for m in &band.members {
                        assert!(
                            !is_straight(&m.row.0),
                            "{flute:?} {d} mm {op_type:?} band holds {}",
                            m.row.0
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 150);
}

/// (d) The rows of the straight series.
#[test]
fn the_straight_rows_are_of_the_straight_series_p24a() {
    let straight: Vec<&VendorObservation> = EMBEDDED_LUT
        .observations
        .iter()
        .filter(|o| row_series(o) == Some(ToolSeries::Straight))
        .collect();
    let count = |prefix: &str| {
        straight
            .iter()
            .filter(|o| o.observation_id.starts_with(prefix))
            .count()
    };
    // 8 diameters x 4 sheets, less the two MDF cells that are not safe.
    assert_eq!(count("onsrud-"), 30);
    // 6 diameters x 4 columns, less the 1/8 in plywood cell.
    assert_eq!(count("freud-straight-"), 23);
    assert_eq!(count("whiteside-c1072-straight-flute"), 1);
    assert_eq!(straight.len(), 54, "no other row is straight");
    // The Freud solid carbide (spiral) rows of the same sheet stay standard.
    assert_eq!(
        row_series(obs("freud-solid-carbide-quarter-hardwood")),
        None
    );
    // The printed 3/4 in hard-wood cells: Onsrud .008-.010 in, Freud
    // .009-.013 in.
    let onsrud = obs("onsrud-hardwood-48-000-3_4-straight");
    assert_eq!(
        (onsrud.chipload_min_mm_tooth, onsrud.chipload_max_mm_tooth),
        (Some(0.2032), Some(0.254))
    );
    let freud = obs("freud-straight-three-quarter-hardwood");
    assert_eq!(
        (freud.chipload_min_mm_tooth, freud.chipload_max_mm_tooth),
        (Some(0.2286), Some(0.3302))
    );
}

/// (e) The straight edge meets the whole depth at one angle: its peak
/// force and its deflection are at or above the helix 30 values.
#[test]
fn a_straight_edge_has_the_higher_peak_force_p24a() {
    let material = white_oak();
    let line = material.force_line().expect("white oak has a force line");
    // The wanaka idx 1 cut: ap 10 mm, ae = the pass ceiling of stepover 5.5
    // on D 18 (P23a: 5.9913 mm), and the chip 0.17913 that ships today.
    let cut = CutPoint {
        ap_mm: 10.0,
        ae_mm: 5.9913,
    };
    let fz = 0.179_13;
    let figures = |flute: BitCutDirection| {
        let tool = flat_2f(18.0, flute);
        let cutter = build_cutter(&tool).expect("a cutter");
        let engagement = Engagement::from_tool_cut(cut, &cutter);
        let force = engagement
            .edge_force(line, fz, 2)
            .expect("a modelled force");
        let deflection = tip_deflection_from_engagement(&cutter, &material, &engagement, fz)
            .expect("a modelled deflection");
        (
            cutter.helix_deg(),
            force.peak_resultant_n,
            force.mean_tangential_n,
            deflection,
        )
    };
    let spiral = figures(BitCutDirection::UpCut);
    let straight = figures(BitCutDirection::Straight);
    eprintln!(
        "P24a probe | helix {} deg: peak {:.2} N, mean tangential {:.2} N, deflection {:.4} mm | \
         helix {} deg: peak {:.2} N, mean tangential {:.2} N, deflection {:.4} mm",
        spiral.0, spiral.1, spiral.2, spiral.3, straight.0, straight.1, straight.2, straight.3
    );
    assert_eq!(straight.0, 0.0);
    assert!(spiral.0 > 0.0, "non-vacuity: the spiral tool has a helix");
    assert!(
        straight.1 >= spiral.1,
        "peak force {} N (straight) vs {} N (helix {})",
        straight.1,
        spiral.1,
        spiral.0
    );
    assert!(
        straight.3 >= spiral.3,
        "the deflection follows the peak force"
    );
    // The mean force (the power) does not read the helix.
    assert!((straight.2 - spiral.2).abs() < 1e-9 * spiral.2.abs().max(1.0));
}
