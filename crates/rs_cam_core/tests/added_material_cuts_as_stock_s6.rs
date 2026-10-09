//! S6 sentries: the "cut as" switch of a stock change
//! (`planning/stock_additions_2026-10-09/PLAN.md` S6).
//!
//! Operator ruling (2026-10-09): "in theory it is just more stock". An added
//! material is cut as the stock material by default, for every gate, every
//! cut metric and the feed modulation. `cut_as = own_material` judges the
//! samples of that material with its own force data, or counts and names
//! them as "not judged" when it has none.
//!
//! The fixture (`tests/fixtures/stock_change_cut_as_s6`) removes a 1 mm slab
//! over a 30 x 30 mm square and fills it back to Z -0.5 with an added
//! material. Two pockets then cut through the fill into the stock below, so
//! the run holds samples of the added material, of the stock, and mixed
//! samples.
//!
//! Each test fails on a stub: a helper that ignores `cut_as` (and judges a
//! slot as its own material) fails the identity and mixed-sample tests; a
//! helper that ignores the slot (judges everything as stock) fails the two
//! own-material tests.

#![allow(
    // SAFETY: test code; a failed fixture is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::stock_change::{CutAs, StockChange};
use rs_cam_core::ids::{StockChangeId, ToolpathId};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{Command, ProjectSession, ReplaceStockChangeArgs, SimulationOptions};
use rs_cam_core::stock::cut_as::EffectiveMaterial;
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::{ToolLoadReport, ToolpathLoadVerdict};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stock_change_cut_as_s6/project.toml")
}

fn resin() -> Material {
    Material::Custom {
        name: "Resin".to_owned(),
        feed_scale_factor: 1.0,
    }
}

fn maple() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// One generated, simulated and modulated run of the fixture, with the
/// fill ("Pour", change 1) set to `material` and `cut_as`.
struct Run {
    session: ProjectSession,
}

impl Run {
    fn new(material: Material, cut_as: CutAs) -> Self {
        let mut session = ProjectSession::load(&fixture()).expect("the fixture loads");
        let pour: StockChange = session.list_setups()[0]
            .stock_changes
            .iter()
            .find(|c| c.id == StockChangeId(1))
            .cloned()
            .expect("the fixture holds the fill");
        let mut change = pour;
        change.material = material;
        change.cut_as = cut_as;
        let _ = session
            .apply(Command::ReplaceStockChange(ReplaceStockChangeArgs {
                setup_index: 0,
                change_id: StockChangeId(1),
                change: Box::new(change),
            }))
            .expect("the fill edit applies");

        let opts = SimulationOptions {
            resolution: session.simulation_resolution_mm(),
            ..SimulationOptions::default()
        };
        assert!(opts.adaptive_feed_modulation, "the default modulates");
        let cancel = AtomicBool::new(false);
        for step in plan(&session, Scope::Project) {
            match step {
                Step::Simulate { .. } => {
                    let _ = session.run_simulation(&opts, &cancel).expect("simulate");
                }
                Step::Generate { index, .. } => {
                    let _ = session.generate_toolpath(index, &cancel).expect("generate");
                }
            }
        }
        let _ = session
            .run_simulation(&opts, &cancel)
            .expect("the closing simulation");
        Self { session }
    }

    fn trace(&self) -> &SimulationCutTrace {
        self.session
            .simulation_result()
            .and_then(|s| s.cut_trace.as_deref())
            .expect("every simulation keeps its trace")
    }

    fn report(&self) -> ToolLoadReport {
        self.session.tool_load_report()
    }

    /// Every G-code line that is not a comment: the moves and their
    /// F-words.
    fn gcode_motion(&self) -> Vec<String> {
        let gcode = rs_cam_core::gcode::export_gcode_checked(
            &self.session,
            Some(self.trace()),
            rs_cam_core::gcode::ToolLoadExportPolicy {
                accept_unmodeled: true,
                accept_exceeded: true,
            },
        )
        .expect("the export runs");
        gcode
            .lines()
            .filter(|l| !l.trim_start().starts_with('(') && !l.trim_start().starts_with(';'))
            .map(str::to_owned)
            .collect()
    }

    fn stock_material(&self) -> Material {
        self.session.stock_config().material.clone()
    }
}

fn f_words(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .flat_map(|l| l.split_whitespace())
        .filter(|w| w.starts_with('F'))
        .map(str::to_owned)
        .collect()
}

fn verdict_for(report: &ToolLoadReport, id: ToolpathId) -> &ToolpathLoadVerdict {
    report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == id)
        .expect("a verdict per toolpath")
}

/// The toolpath ids whose samples cut the added material (slot != 0).
fn toolpaths_cutting_the_fill(trace: &SimulationCutTrace) -> Vec<ToolpathId> {
    let mut ids: Vec<ToolpathId> = trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && !s.material_slot.is_stock())
        .map(|s| s.toolpath_id)
        .collect();
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    ids
}

/// The default cuts the fill as the stock: the verdicts and the modulated
/// F-words are identical to a run whose fill is plain stock material, built
/// the same way (the same Add change, the same slot, the stock material).
#[test]
fn a_default_fill_gives_the_verdicts_and_feeds_of_plain_stock_s6() {
    let filled = Run::new(resin(), CutAs::StockMaterial);
    let stock = filled.stock_material();
    let plain = Run::new(stock.clone(), CutAs::StockMaterial);

    // Non-vacuity: the pockets cut the fill, the stock gates measure, and
    // the modulation rewrote feeds.
    let cutting = toolpaths_cutting_the_fill(filled.trace());
    assert!(!cutting.is_empty(), "the pockets cut the fill");
    assert!(
        stock.force_line().is_ok(),
        "the stock material has force data, so power and deflection measure"
    );
    let report = filled.report();
    for id in &cutting {
        let v = verdict_for(&report, *id);
        assert!(
            !v.power.is_unmodeled(),
            "the power gate measures toolpath {id:?}: {:?}",
            v.power
        );
        assert!(v.material_split.is_none(), "the default makes no split");
    }
    assert!(
        !filled.trace().modulated_feeds.is_empty(),
        "the modulation ran"
    );
    assert!(report.summary(|_| None).not_judged.is_empty());

    // The identity: the same report, the same modulated motion.
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::to_value(plain.report()).unwrap(),
        "a default fill gives the verdicts of plain stock"
    );
    assert_eq!(
        filled.trace().modulated_feeds,
        plain.trace().modulated_feeds,
        "a default fill gives the modulated feeds of plain stock"
    );
    let filled_motion = filled.gcode_motion();
    let plain_motion = plain.gcode_motion();
    assert!(!f_words(&filled_motion).is_empty());
    assert_eq!(f_words(&filled_motion), f_words(&plain_motion));
    assert_eq!(filled_motion, plain_motion);
}

/// A sample that cut the fill and the stock is a normal stock sample under
/// the default: the helper judges it as the stock material.
#[test]
fn a_mixed_sample_under_the_default_is_a_stock_sample_s6() {
    let run = Run::new(resin(), CutAs::StockMaterial);
    let stock = run.stock_material();
    let trace = run.trace();
    let mixed: Vec<_> = trace
        .samples
        .iter()
        .filter(|s| s.cuts_several_materials)
        .collect();
    assert!(!mixed.is_empty(), "the pockets cut fill and stock together");
    assert!(
        mixed.iter().any(|s| !s.material_slot.is_stock()),
        "a mixed sample whose main slot is the fill exists"
    );
    for s in &mixed {
        assert_eq!(
            trace.effective_material_for_sample(s, &stock),
            EffectiveMaterial::Judged(&stock)
        );
    }
    // Every sample of the run is judged as the stock.
    for s in &trace.samples {
        assert!(trace.judged_as(s, &stock, &stock));
    }
}

/// `own_material` with force data: the samples of the fill are judged with
/// the fill's own force line, in their own verdict.
#[test]
fn own_material_with_force_data_uses_it_s6() {
    let run = Run::new(maple(), CutAs::OwnMaterial);
    let stock = run.stock_material();
    assert_ne!(stock, maple(), "the fill is not the stock material");
    let report = run.report();
    let trace = run.trace();
    let cutting = toolpaths_cutting_the_fill(trace);
    assert!(!cutting.is_empty());

    let mut judged_own = 0;
    let mut reasons = Vec::new();
    for id in &cutting {
        let v = verdict_for(&report, *id);
        let split = v
            .material_split
            .as_deref()
            .expect("own material splits the populations");
        assert!(split.not_judged.is_empty(), "maple has force data");
        let own = split
            .own
            .iter()
            .find(|o| o.material == maple().label())
            .expect("the maple population has its own verdict");
        assert!(own.samples > 0);
        let fill_samples = trace
            .samples
            .iter()
            .filter(|s| s.toolpath_id == *id && !s.material_slot.is_stock())
            .count();
        assert_eq!(own.samples, fill_samples, "the population is the fill");

        // The own verdict is the power gate run with MAPLE on the fill
        // population, and it is not the gate run with the stock material.
        let view = trace.population_view(*id, |s| !s.material_slot.is_stock());
        let tc = run
            .session
            .toolpath_configs()
            .iter()
            .find(|tc| tc.id == *id)
            .unwrap();
        let tool = rs_cam_core::compute::cutter::build_cutter(
            run.session
                .get_tool(rs_cam_core::compute::tool_config::ToolId(tc.tool_id))
                .unwrap(),
        );
        let maple = maple();
        // The spans the report reads (`gcode::project_load_report`).
        let index = run
            .session
            .toolpath_configs()
            .iter()
            .position(|t| t.id == *id)
            .unwrap();
        let spans = run
            .session
            .get_result(index)
            .filter(|r| r.annotated().spans_valid)
            .map(|r| r.annotated().spans.as_slice());
        let ctx_for = |material| rs_cam_core::tool_load::ToolpathLoadContext {
            toolpath_id: *id,
            tool: &tool,
            material,
            operation_family: rs_cam_core::feeds::vendor_lut::LutOperationFamily::Pocket,
            pass_role: rs_cam_core::feeds::vendor_lut::LutPassRole::Roughing,
            operation_feed_rate_mm_min: tc.operation.feed_rate(),
            operation_kind: tc.operation.op_type(),
            spans,
            drill_op: None,
        };
        let machine = run.session.machine();
        let env = rs_cam_core::tool_load::GateEnv {
            sim_trace: Some(&view),
            machine: Some(machine),
            tolerance: &rs_cam_core::tool_load::ToleranceBands::default(),
        };
        let maple_power = rs_cam_core::tool_load::power::evaluate(&ctx_for(&maple), &env);
        let stock_power = rs_cam_core::tool_load::power::evaluate(&ctx_for(&stock), &env);
        reasons.push(format!(
            "{id:?}: own {:?} / view {:?}",
            own.power, maple_power
        ));
        if !maple_power.is_unmodeled() {
            judged_own += 1;
            assert_eq!(
                serde_json::to_value(&own.power).unwrap(),
                serde_json::to_value(&maple_power).unwrap(),
                "the own verdict is the gate run with the fill's force data"
            );
            assert_ne!(
                serde_json::to_value(&maple_power).unwrap(),
                serde_json::to_value(&stock_power).unwrap(),
                "the force data decides the reading"
            );
        }
    }
    assert!(
        judged_own > 0,
        "at least one maple population measured power: {reasons:#?}"
    );
}

/// `own_material` without force data: the samples of the fill are not
/// judged, and the report, the summary and the diagnostics count and name
/// them.
#[test]
fn own_material_without_force_data_is_not_judged_and_named_s6() {
    let run = Run::new(resin(), CutAs::OwnMaterial);
    let report = run.report();
    let trace = run.trace();
    let cutting = toolpaths_cutting_the_fill(trace);
    assert!(!cutting.is_empty());

    let summary = report.summary(|id| Some(format!("tp{}", id.0)));
    for id in &cutting {
        let v = verdict_for(&report, *id);
        let fill_samples = trace
            .samples
            .iter()
            .filter(|s| s.toolpath_id == *id && !s.material_slot.is_stock())
            .count();
        assert!(fill_samples > 0);
        let counts = v.not_judged();
        assert_eq!(counts.len(), 1, "one not-judged material");
        assert_eq!(counts[0].material, "Resin");
        assert_eq!(counts[0].samples, fill_samples);
        assert_eq!(
            counts[0].label(),
            format!("{fill_samples} samples not judged: no force data for Resin")
        );
        assert!(
            v.material_split.as_deref().unwrap().own.is_empty(),
            "no judged own material"
        );

        // The summary names it per toolpath.
        let entry = summary
            .not_judged
            .iter()
            .find(|e| e.toolpath_id == *id)
            .expect("the summary lists the toolpath");
        assert_eq!(entry.material, "Resin");
        assert_eq!(entry.samples, fill_samples);
        assert!(entry.label().contains("no force data for Resin"));

        // The diagnostics carry one not-judged finding with the same text.
        let index = run
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == *id)
            .unwrap();
        let diags = run.session.diagnose_toolpath(index).unwrap();
        let found: Vec<_> = diags
            .iter()
            .filter(|d| d.id.0 == rs_cam_core::diagnostics::ids::LOAD_MATERIAL_NOT_JUDGED)
            .collect();
        assert_eq!(found.len(), 1, "{diags:#?}");
        assert_eq!(found[0].message, counts[0].label());
    }

    // The stock population verdict excludes the fill: it equals the gates
    // run over the stock samples alone.
    let stock = run.stock_material();
    for s in trace.samples.iter().filter(|s| !s.material_slot.is_stock()) {
        assert!(matches!(
            trace.effective_material_for_sample(s, &stock),
            EffectiveMaterial::NotJudged(_)
        ));
    }
}

/// The switch is part of the change's effect: an edit of `cut_as` drops the
/// simulation, and the project file keeps it.
#[test]
fn cut_as_is_part_of_the_change_and_the_file_s6() {
    let session = ProjectSession::load(&fixture()).expect("the fixture loads");
    let pour = session.list_setups()[0]
        .stock_changes
        .iter()
        .find(|c| c.id == StockChangeId(1))
        .cloned()
        .unwrap();
    assert_eq!(
        pour.cut_as,
        CutAs::OwnMaterial,
        "the file sets own_material"
    );
    let slab = session.list_setups()[0]
        .stock_changes
        .iter()
        .find(|c| c.id == StockChangeId(0))
        .cloned()
        .unwrap();
    assert_eq!(
        slab.cut_as,
        CutAs::StockMaterial,
        "a missing key is the default"
    );

    let dir = std::env::temp_dir().join(format!("rs_cam_cut_as_s6_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        fixture().parent().unwrap().join("square.svg"),
        dir.join("square.svg"),
    )
    .unwrap();
    let path = dir.join("project.toml");
    session.save(&path).expect("the project saves");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("cut_as = \"own_material\""), "{text}");
    let reloaded = ProjectSession::load(&path).expect("the saved project loads");
    assert_eq!(
        reloaded.list_setups()[0].stock_changes,
        session.list_setups()[0].stock_changes
    );
    let _ = std::fs::remove_dir_all(&dir);
}
