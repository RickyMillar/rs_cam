//! Feeds contract P13c (operator ruling O1, 2026-11-05): the hobby force
//! budget is derived from the printed hobby full-slot rows, not typed.
//!
//! The claims:
//!
//! 1. Each row of `feeds::force_budget::HOBBY_SLOT_POINTS` is in its chart
//!    file at its line, verbatim, and the line prints each number the row
//!    holds (inch values converted at 25.4 mm).
//! 2. The test recomputes the peak engaged-edge resultant of each printed
//!    cut through `Engagement::edge_force` (a flat end mill in a full slot,
//!    the repo default helix) and the budget of the hobby default machine is
//!    the largest of them, to the bit. The largest is the Sienci
//!    "Softwood, Soft Plywood, MDF" row read on the MDF line.
//! 3. The F201 registry holds the budget as a `Cited` row with that value.
//! 4. The machine doors apply the class rule: a hobby `[rigidity]` block
//!    with no `force_budget_n` reads the derived budget; an industrial one
//!    refuses; the deleted keys refuse with their P13c fix.
//!
//! Red before P13c (HEAD 4efcb4eaa): `feeds::force_budget` and
//! `RigidityProfile::force_budget_n` do not exist, so the file does not
//! compile.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_hobby_force_budget_recomputes_from_the_printed_rows_p13c
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;

use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::force_budget::{HOBBY_SLOT_POINTS, HobbySlotPoint, hobby_budget};
use rs_cam_core::machine::{
    INDUSTRIAL_NEEDS_FORCE_BUDGET, MachineClass, MachineProfile, RigidityProfile,
};
use rs_cam_core::material::ConstantSource;

const INCH_MM: f64 = 25.4;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The printed tokens of each row: (id, tokens the line prints, the row
/// numbers those tokens give: D, Z, ap, feed, rpm).
fn printed(id: &str) -> (Vec<&'static str>, [f64; 5]) {
    let quarter = 0.25 * INCH_MM;
    match id {
        "sienci_quarter_softwood" | "sienci_quarter_mdf" => (
            vec!["1/4\"", "2740", "5.4", "12000"],
            [quarter, 2.0, 5.4, 2740.0, 12000.0],
        ),
        "sienci_quarter_hardwood" => (
            vec!["1/4\"", "2740", "2.5", "12000"],
            [quarter, 2.0, 2.5, 2740.0, 12000.0],
        ),
        "carbide3d_s3_mdf" => (
            vec![".3\u{2033}", "17000", "80"],
            [quarter, 3.0, 0.3 * INCH_MM, 80.0 * INCH_MM, 17000.0],
        ),
        "carbide3d_s3_pine" => (
            vec![".4\u{2033}", "21000", "75"],
            [quarter, 3.0, 0.4 * INCH_MM, 75.0 * INCH_MM, 21000.0],
        ),
        "carbide3d_s3_mahogany" => (
            vec![".1\u{2033}", "18950", "65"],
            [quarter, 3.0, 0.1 * INCH_MM, 65.0 * INCH_MM, 18950.0],
        ),
        other => panic!("an unknown row {other}"),
    }
}

/// The peak resultant of a printed cut, computed here (not through the core
/// helper): a flat end mill of the printed D in a full slot.
fn peak_n(p: &HobbySlotPoint) -> f64 {
    let line = p
        .material
        .material()
        .force_line()
        .expect("every hobby row's material has a force line");
    let fz = p.feed_mm_min / (p.rpm * f64::from(p.flutes));
    Engagement::from_cut(
        CutPoint {
            ap_mm: p.ap_mm,
            ae_mm: p.diameter_mm,
        },
        ToolGeometryHint::Flat,
        p.diameter_mm,
        Some(rs_cam_core::tool::DEFAULT_HELIX_DEG),
    )
    .edge_force(line, fz, p.flutes)
    .expect("the edge force answers")
    .peak_resultant_n
}

#[test]
fn each_hobby_row_is_printed_in_its_chart_p13c() {
    assert_eq!(HOBBY_SLOT_POINTS.len(), 6, "the six step 0 hobby points");
    for p in &HOBBY_SLOT_POINTS {
        let text = std::fs::read_to_string(repo_root().join(p.path))
            .unwrap_or_else(|e| panic!("{}: {} is not in the repo: {e}", p.id, p.path));
        let line = text
            .lines()
            .nth(p.line_no - 1)
            .unwrap_or_else(|| panic!("{}: no line {}", p.id, p.line_no));
        assert_eq!(line.trim(), p.line, "{}: the chart line moved", p.id);
        let (tokens, numbers) = printed(p.id);
        let words: Vec<&str> = p.line.split_whitespace().collect();
        for t in tokens {
            assert!(words.contains(&t), "{}: the line does not print {t}", p.id);
        }
        let held = [
            p.diameter_mm,
            f64::from(p.flutes),
            p.ap_mm,
            p.feed_mm_min,
            p.rpm,
        ];
        for (h, n) in held.iter().zip(numbers) {
            assert!((h - n).abs() < 1e-12, "{}: holds {h}, prints {n}", p.id);
        }
    }
}

#[test]
fn the_hobby_budget_is_the_largest_printed_peak_p13c() {
    let peaks: Vec<(String, f64)> = HOBBY_SLOT_POINTS
        .iter()
        .map(|p| (p.id.to_owned(), peak_n(p)))
        .collect();
    let (max_id, max_n) = peaks
        .iter()
        .cloned()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("six rows");
    let budget = hobby_budget();
    assert_eq!(budget.force_n.to_bits(), max_n.to_bits(), "{peaks:?}");
    assert_eq!(HOBBY_SLOT_POINTS[budget.binding].id, max_id);
    assert_eq!(max_id, "sienci_quarter_mdf", "{peaks:?}");
    assert_eq!(
        RigidityProfile::default().force_budget_n.to_bits(),
        max_n.to_bits()
    );
    assert_eq!(
        MachineProfile::generic_wood_router()
            .rigidity
            .force_budget_n
            .to_bits(),
        max_n.to_bits()
    );
    // The budget is a chart force, not a typed number: the core helper and
    // this recomputation agree on every row.
    for (p, (_, n)) in HOBBY_SLOT_POINTS.iter().zip(&peaks) {
        let core = p.peak_force_n().expect("the core helper answers");
        assert_eq!(core.to_bits(), n.to_bits(), "{}", p.id);
    }
}

#[test]
fn the_budget_is_a_cited_f201_row_p13c() {
    let rows = rs_cam_core::feeds::formula_constants();
    let row = rows
        .iter()
        .find(|r| r.name == "machine.RigidityProfile.hobby.force_budget_n")
        .expect("the F201 row of the hobby budget");
    assert_eq!(row.value.to_bits(), hobby_budget().force_n.to_bits());
    match row.source {
        ConstantSource::Cited { path, printed } => {
            let p = &HOBBY_SLOT_POINTS[hobby_budget().binding];
            assert_eq!(path, p.path);
            assert_eq!(printed, p.line);
        }
        ConstantSource::Unsourced { claim } => panic!("the budget row is Unsourced: {claim}"),
    }
}

fn machine_table(edit: impl FnOnce(&mut toml::Table)) -> toml::Table {
    let mut t = toml::Table::try_from(MachineProfile::default()).expect("the default serialises");
    edit(&mut t);
    t
}

fn load(name: &str, t: &toml::Table) -> Result<MachineProfile, String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("p13c_budget_{name}"));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(
        dir.join(format!("{name}.toml")),
        toml::to_string(t).expect("text"),
    )
    .expect("write");
    rs_cam_core::io::machine_library::load_from(&dir, name).map_err(|e| format!("{e:?} / {e}"))
}

fn rigidity(t: &mut toml::Table) -> &mut toml::Table {
    match t.get_mut("rigidity") {
        Some(toml::Value::Table(r)) => r,
        _ => panic!("the default states a rigidity block"),
    }
}

#[test]
fn the_class_rule_gives_or_refuses_the_budget_p13c() {
    // A hobby block with no budget reads the derived hobby budget.
    let t = machine_table(|t| {
        t.insert("class".to_owned(), toml::Value::String("hobby".to_owned()));
        let _ = rigidity(t).remove("force_budget_n");
    });
    let m = load("hobby_no_budget", &t).expect("a hobby block with no budget loads");
    assert_eq!(m.class, MachineClass::Hobby);
    assert_eq!(
        m.rigidity.force_budget_n.to_bits(),
        hobby_budget().force_n.to_bits()
    );
    // An industrial block with no budget refuses: no default.
    let t = machine_table(|t| {
        t.insert(
            "class".to_owned(),
            toml::Value::String("industrial".to_owned()),
        );
        let _ = rigidity(t).remove("force_budget_n");
    });
    let e = load("industrial_no_budget", &t).expect_err("an industrial block states its budget");
    assert!(e.contains(INDUSTRIAL_NEEDS_FORCE_BUDGET), "{e}");
    // The deleted keys refuse.
    for key in ["doc_roughing_factor", "doc_finishing_factor"] {
        let t = machine_table(|t| {
            let _ = rigidity(t).insert(key.to_owned(), toml::Value::Float(0.25));
        });
        let e = load(&format!("old_{key}"), &t).expect_err("the key is deleted");
        assert!(e.contains(key), "{e}");
    }
}
