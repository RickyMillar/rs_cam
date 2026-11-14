//! P17 (lead ruling 2, 2026-11-06): headless snapshots of the envelope
//! section at the narrow and the normal inspector widths.
//!
//! The app has one INK palette (ruling 1: dark only), so one image per
//! width is the whole set. The test renders the rivmap100 3D Rough
//! envelope section through `egui_kittest` with the wgpu renderer (a
//! software adapter when the machine has one) inside a right panel of 240,
//! 340 and 420 points, and writes `envelope_<width>.png` into the directory
//! `RS_CAM_P17_SNAPSHOT_DIR` names. A fourth image, `envelope_340_draft.png`,
//! holds a candidate (the draft of a drag): the ghost ring and the
//! proposal diamond paint with the dot. A fifth, `envelope_340_hard_top.png`,
//! is the fixture Pocket at a 1000 mm depth: the depth range reaches the
//! cutting length, so the frame top is a named hard stop.
//!
//! Every test is `#[ignore]`: it writes files and needs a GPU adapter. Run
//! it by hand:
//!
//! ```text
//! RS_CAM_P17_SNAPSHOT_DIR=planning/feeds_contract_2026-11-03/p17_visual \
//!   cargo test -p rs_cam_viz --test p17_visual_snapshots -- --ignored
//! ```
//!
//! Without the variable the test prints a line and passes. When wgpu finds
//! no headless adapter, the test prints the error and fails: that is a
//! blocker for the lead, not a defect of the chart.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the skip and the blocker print"
)]

mod envelope_k2b;

use std::path::{Path, PathBuf};

use envelope_k2b::ID;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::{CutPoint, Edge, LimitKind, SafeEnvelope};
use rs_cam_core::ids::ToolId;
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::material::Material;
use rs_cam_core::tool_load::cut_force::CutPoint3;
use rs_cam_core::tool_load::force_field::{
    BudgetRole, CONTOUR_SHARES, ForceLevel, ForcePlane, contours,
};
use rs_cam_core::tool_load::verdict::{
    BoundSource, CriterionKind, GatePopulation, LoadState, PopulationUnit,
};
use rs_cam_core::tool_load::{
    DistributionMetric, DistributionOutcome, Histogram, MetricDistribution, PopulationSample,
};
use rs_cam_viz::app::{SidePanelEdge, side_panel};
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::state::simulation::{CUT_METRIC_ORDER, CutMetricCard, CutMetricSet};
use rs_cam_viz::ui::feeds::envelope::{EnvelopeScene, draw_envelope};
use rs_cam_viz::ui::sim_cut_chart::{GRID_COLUMNS, GRID_ROWS};
use rs_cam_viz::ui::sim_diagnostics::{
    CutMetricsBody, draw_cut_chart_section, draw_cut_metric_cards,
};
use rs_cam_viz::ui::tokens;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml"
);

/// The height of the harness screen: the header row, the plot and some
/// air under it.
const SCREEN_HEIGHT: f32 = 220.0;

/// The widths of the inspector rail: the Simulation rail, the default and
/// the maximum (`app.rs`; R4, operator ruling 2026-10-30).
const WIDTHS: [f32; 3] = [240.0, 340.0, 420.0];

/// The central area left of the panel, in points: a side panel needs a
/// central panel beside it, and the saved image crops it away.
const CENTRAL_WIDTH: f32 = 60.0;

/// The scale of the render: two pixels per point, so the critique reads
/// the glyphs.
const PIXELS_PER_POINT: f32 = 2.0;

/// The rivmap100 3D Rough envelope at the fixture point.
fn rivmap_env() -> (OperationConfig, SafeEnvelope) {
    let session = rs_cam_core::session::ProjectSession::load(Path::new(FIXTURE))
        .expect("the rivmap100 fixture loads");
    let tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .clone();
    let env = session
        .safe_envelope(&tc)
        .expect("a tool")
        .expect("a backed envelope");
    (tc.operation, env)
}

/// The directory of the snapshots, or `None` to skip.
fn snapshot_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("RS_CAM_P17_SNAPSHOT_DIR")?;
    Some(PathBuf::from(dir))
}

/// The fixture Pocket with a stated depth of 1000 mm (the p5c hard-stop
/// arm): the depth range reaches the cutting length, so the frame top
/// names its hard stop and the depth edge tick paints.
fn hard_top_env() -> (OperationConfig, SafeEnvelope) {
    let mut config = rs_cam_core::compute::fixture_values::fixture_op(
        rs_cam_core::compute::catalog::OperationType::Pocket,
    );
    if let OperationConfig::Pocket(c) = &mut config {
        c.depth_per_pass = Some(1000.0);
    }
    envelope_k2b::envelope_of(config).expect("the Pocket is backed")
}

/// Render the envelope section of `(config, env)` at `width` and save it
/// as `name` in `dir`.
fn snapshot(
    dir: &Path,
    name: &str,
    width: f32,
    (config, env): (OperationConfig, SafeEnvelope),
    candidate: Option<CutPoint>,
) {
    let mut candidates = EnvelopeCandidates::default();
    if let Some(c) = candidate {
        candidates.set(ID, c);
    }
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(width + CENTRAL_WIDTH, SCREEN_HEIGHT))
            .with_pixels_per_point(PIXELS_PER_POINT)
            .wgpu()
            .build_ui(move |ui| {
                // The app's own inspector panel (`app::side_panel`): the
                // scroll area, the gutter, the width rule.
                side_panel(ui, SidePanelEdge::Right, "p17_snapshot", width, |ui| {
                    let mut events = Vec::new();
                    let _ = draw_envelope(
                        ui,
                        ID,
                        &config,
                        Some(&env),
                        None,
                        &mut candidates,
                        &mut events,
                    );
                });
            })
    }));
    let mut harness = match built {
        Ok(harness) => harness,
        Err(err) => {
            let text = err
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "no message".to_owned());
            eprintln!("P17 BLOCKER: wgpu found no headless adapter: {text}");
            panic!("P17 blocker: no headless wgpu adapter: {text}");
        }
    };
    tokens::apply(&harness.ctx);
    tokens::apply_fonts(&harness.ctx);
    let _ = harness.run();
    let image = harness
        .render()
        .unwrap_or_else(|err| panic!("P17 blocker: the wgpu render failed: {err}"));
    // The panel alone: the central area to its left is cropped away.
    let left = (CENTRAL_WIDTH * PIXELS_PER_POINT).round() as u32;
    let image =
        image::imageops::crop_imm(&image, left, 0, image.width() - left, image.height()).to_image();
    let path = dir.join(name);
    image
        .save(&path)
        .unwrap_or_else(|err| panic!("save {}: {err}", path.display()));
    eprintln!("P17 snapshot: {}", path.display());
}

#[test]
#[ignore = "writes PNG files and needs a wgpu adapter; set RS_CAM_P17_SNAPSHOT_DIR"]
fn the_envelope_at_each_rail_width_p17() {
    let Some(dir) = snapshot_dir() else {
        eprintln!("RS_CAM_P17_SNAPSHOT_DIR is not set: no snapshot");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("mkdir {}: {err}", dir.display()));
    for width in WIDTHS {
        snapshot(
            &dir,
            &format!("envelope_{width:.0}.png"),
            width,
            rivmap_env(),
            None,
        );
    }
    // The draft: a candidate at half the proposed depth and 1.5 x its
    // width, so the ghost and the diamond paint beside the dot.
    let (_, env) = rivmap_env();
    let p = env.proposal.get().point().expect("a proposed depth");
    let draft = CutPoint {
        ap_mm: p.ap_mm * 0.5,
        ae_mm: p.ae_mm * 1.5,
    };
    snapshot(
        &dir,
        "envelope_340_draft.png",
        340.0,
        rivmap_env(),
        Some(draft),
    );
    // The hard top: the frame top names its stop and the depth edge tick
    // paints (the y arm of the D5 rule).
    snapshot(
        &dir,
        "envelope_340_hard_top.png",
        340.0,
        hard_top_env(),
        None,
    );
}

// ── Agent C: the sim cards and the width x depth chart ──────────────────

/// The height of the cards screen: four cards, the readings and the
/// hairlines.
const CARDS_HEIGHT: f32 = 500.0;

/// The height of the chart screen: the hairline, the header row, the
/// plot and the legend (P17b: two legend rows at the 240 rail, where the
/// ramp swatch wraps the playhead entry).
const CHART_HEIGHT: f32 = 300.0;

/// The samples of the prototype set (explainer P13c section 9).
const SAMPLE_COUNT: usize = 500;

/// The shares of the four sample classes of the prototype (explainer P13c
/// section 9, `proto_share_*`): steady passes, corners, the opening slot,
/// the ramp entry. The rest of the time is steady.
const SHARE_CORNER: f64 = 0.12;
const SHARE_SLOT: f64 = 0.05;
const SHARE_RAMP: f64 = 0.03;

/// The width jitter of a steady pass, each way (`proto_jitter_pct`).
const JITTER: f64 = 0.02;

/// Round 2 (lead review): a few samples deeper than the envelope's own
/// view, at one and a half times the planned depth, so the chart's depth
/// range shows that it holds every sample. Not an explainer class; taken
/// from the steady share.
const SHARE_DEEP: f64 = 0.02;
const DEEP_FACTOR: f64 = 1.5;

/// A fixed-seed generator (a 64-bit LCG), so the snapshot is the same
/// each run.
struct Lcg(u64);

impl Lcg {
    fn next_unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// The `mm` of an `AeCeiling` edge of `env`'s limit `kind`.
fn ae_ceiling(env: &SafeEnvelope, kind: LimitKind) -> Option<f64> {
    match env.limit(kind)?.edge {
        Edge::AeCeiling { mm } => Some(mm),
        Edge::ApCeiling { .. }
        | Edge::ApFloor { .. }
        | Edge::AeFloor { .. }
        | Edge::ApCurve { .. }
        | Edge::PerPoint
        | Edge::AeCurve { .. }
        | Edge::Unmodelled { .. } => None,
    }
}

/// A Pocket-like set: the fixture Pocket envelope (a 6 mm end mill, the
/// default machine and material; the Pocket is under the force budget), a
/// prototype sample population of four classes, and the four cards built
/// from core's own laws at each sample: the peak chip `fz sin(min(psi,
/// 90 deg))` (the explainer's rule), the envelope's power at the sample
/// point (the speeds grid), the tip deflection
/// (`predict::tip_deflection_from_engagement`) and the peak edge force
/// (`Engagement::edge_force`). The chip band is the calculator's own
/// (`FeedsResult::chipload_bounds`), the power ceiling the grid's
/// `available_kw`, the deflection limit the finish target
/// (`deflection::EXCEEDS_BOUND_MM`), the force line the machine's rule.
fn sim_set() -> CutMetricSet {
    let config = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    let (_, env) = envelope_k2b::envelope_of(config.clone()).expect("the Pocket is backed");
    let mut tool_cfg = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    tool_cfg.diameter = 6.0;
    let tool = rs_cam_core::compute::cutter::build_cutter(&tool_cfg).expect("a cutter");
    let machine = MachineProfile::default();
    let material = Material::default();
    let line = material
        .force_line()
        .expect("the default material has a force line");
    let budget = machine
        .rigidity
        .force_budget_of(
            config
                .cuts(&rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&config))
                .body_facts(),
        )
        .ok()
        .map(|DepthCapRule { budget_n }| budget_n);
    let diameter = ae_ceiling(&env, LimitKind::Diameter).unwrap_or(tool_cfg.diameter);
    let corner = ae_ceiling(&env, LimitKind::CornerCoverage).unwrap_or(diameter);
    let planned = env
        .stated
        .or(env.proposal.get().point())
        .expect("the fixture states or proposes a point");
    // The chip band of the calculator: the row's band, else its one
    // printed chip as the ceiling (a point row).
    let feeds = rs_cam_core::feeds::calculate(&rs_cam_core::feeds::FeedsInput::of_operation(
        &config,
        &tool_cfg,
        &material,
        &machine,
        rs_cam_core::feeds::embedded_vendor_lut(),
        rs_cam_core::feeds::SpindleStrategy::default(),
    ));
    let (chip_floor, chip_ceiling) = match (feeds.chipload_bounds, feeds.chipload_point_mm) {
        (Some(b), _) => (Some(b.min_mm_per_tooth), Some(b.max_mm_per_tooth)),
        (None, point) => (None, point),
    };
    // The feed per tooth of the prototype: the calculator's chip, so the
    // slot samples sit at the printed chip and the narrower cuts under it.
    let fz = feeds.chip_load_mm;
    let available_kw = env.speeds.points.first().map_or(0.0, |p| p.available_kw);

    // The four classes, by index: the ramp, the slot and the corners first
    // (the entry of a pocket), then the steady passes.
    let mut rng = Lcg(0x5EED_0017);
    let n_ramp = (SAMPLE_COUNT as f64 * SHARE_RAMP).round() as usize;
    let n_slot = (SAMPLE_COUNT as f64 * SHARE_SLOT).round() as usize;
    let n_corner = (SAMPLE_COUNT as f64 * SHARE_CORNER).round() as usize;
    let n_deep = (SAMPLE_COUNT as f64 * SHARE_DEEP).round() as usize;
    let mut samples: Vec<CutPoint3> = Vec::with_capacity(SAMPLE_COUNT);
    let mut chip: Vec<PopulationSample> = Vec::new();
    let mut power: Vec<PopulationSample> = Vec::new();
    let mut deflection: Vec<PopulationSample> = Vec::new();
    let mut force: Vec<PopulationSample> = Vec::new();
    for i in 0..SAMPLE_COUNT {
        let (ae, ap) = if i < n_ramp {
            (diameter, planned.ap_mm * (0.1 + 0.9 * rng.next_unit()))
        } else if i < n_ramp + n_slot {
            (diameter, planned.ap_mm)
        } else if i < n_ramp + n_slot + n_corner {
            (
                planned.ae_mm + (corner - planned.ae_mm).max(0.0) * rng.next_unit(),
                planned.ap_mm,
            )
        } else if i < n_ramp + n_slot + n_corner + n_deep {
            (planned.ae_mm, planned.ap_mm * DEEP_FACTOR)
        } else {
            (
                planned.ae_mm * (1.0 + JITTER * (2.0 * rng.next_unit() - 1.0)),
                planned.ap_mm,
            )
        };
        let engagement = Engagement::from_tool_cut(
            CutPoint {
                ap_mm: ap,
                ae_mm: ae,
            },
            &tool,
        );
        let force_n = engagement
            .edge_force(line, fz, tool.flute_count)
            .map(|f| f.peak_resultant_n);
        let weight_s = 1.0;
        samples.push(CutPoint3 {
            ae_mm: ae,
            ap_mm: ap,
            force_n,
            weight_s,
            move_index: i,
            sample_index: i,
        });
        let sample = |value: f64| PopulationSample {
            value,
            weight_s,
            move_index: i,
            sample_index: i,
        };
        chip.push(sample(
            fz * engagement.psi_rad().min(std::f64::consts::FRAC_PI_2).sin(),
        ));
        if let Some(kw) = env.speeds.speeds_at(ap, ae).and_then(|s| s.power_kw) {
            power.push(sample(kw));
        }
        if let Some(d) = rs_cam_core::feeds::predict::tip_deflection_from_engagement(
            &tool,
            &material,
            &engagement,
            fz,
        ) {
            deflection.push(sample(d));
        }
        if let Some(f) = force_n {
            force.push(sample(f));
        }
    }

    let card = |metric: DistributionMetric,
                population: &[PopulationSample],
                floor: Option<f64>,
                ceiling: Option<f64>,
                bound_source: Option<BoundSource>| {
        let histogram = Histogram::build(
            population,
            floor,
            ceiling,
            rs_cam_core::tool_load::distribution::DEFAULT_BIN_COUNT,
        );
        let peak = population
            .iter()
            .map(|s| s.value)
            .fold(f64::NEG_INFINITY, f64::max);
        let state = ceiling.map(|c| {
            if peak > c {
                LoadState::Exceeds
            } else {
                LoadState::Within
            }
        });
        CutMetricCard {
            metric,
            outcome: DistributionOutcome::Measured(Box::new(MetricDistribution {
                metric,
                state,
                bound_source,
                population: GatePopulation::new(
                    population.len(),
                    SAMPLE_COUNT,
                    PopulationUnit::Samples,
                ),
                histogram,
            })),
            series: population.iter().map(|s| (s.move_index, s.value)).collect(),
        }
    };
    let cards = vec![
        card(
            DistributionMetric::Criterion(CriterionKind::Chipload),
            &chip,
            chip_floor,
            chip_ceiling,
            None,
        ),
        card(
            DistributionMetric::Criterion(CriterionKind::Power),
            &power,
            None,
            (available_kw > 0.0).then_some(available_kw),
            None,
        ),
        card(
            DistributionMetric::Criterion(CriterionKind::Deflection),
            &deflection,
            None,
            Some(rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM),
            None,
        ),
        card(
            DistributionMetric::CutForce,
            &force,
            None,
            budget,
            budget.map(|budget_n| BoundSource::MachineForceBudget {
                budget_n,
                ae_mm: planned.ae_mm,
                fz_mm: fz,
                depth_mm: planned.ap_mm,
            }),
        ),
    ];
    assert_eq!(
        cards.iter().map(|c| c.metric).collect::<Vec<_>>(),
        CUT_METRIC_ORDER.to_vec(),
        "the set holds the four cards in the section's order"
    );
    // P17b: the iso-force field at the same advance per tooth the samples
    // read, over the envelope scene's own view top (the state builder's
    // rule); the contours show on the over-budget image.
    // P29-10 10g: the core plane (the field and its level lines). The
    // fixture states the advance, so the plane reads it at one tooth per
    // rev (feed = fz, rpm = 1, flutes = the tool's): the same fz.
    let force_plane = budget.and_then(|budget_n| {
        ForcePlane::build(
            &tool,
            &material,
            fz * f64::from(tool.flute_count),
            1.0,
            diameter,
            EnvelopeScene::of(&env, None).ap_range.1,
            budget_n,
            BudgetRole::Limit,
        )
    });
    CutMetricSet {
        toolpath_id: ID,
        cards,
        envelope: Some(std::sync::Arc::new(env)),
        samples,
        diameter_mm: diameter,
        budget_n: budget,
        force_plane,
    }
}

/// Round 3 (lead review): the chart images draw the rivmap100 3D Rough
/// envelope above a cut at its planned depth. Before P29-4 it had two flat
/// depth limits (the vendor DOC and the rigidity DOC), so two off-scale
/// markers showed; since P29-4 (ruling R1) the rigidity limit is the
/// force-budget curve, and only the vendor DOC is flat. The samples are the
/// four prototype classes about that envelope's planned point, with no
/// force (the set states no budget line and no dot). The cards stay the
/// Pocket set's.
fn rough_chart_set(pocket: &CutMetricSet) -> CutMetricSet {
    let (_, env) = rivmap_env();
    let diameter = ae_ceiling(&env, LimitKind::Diameter).unwrap_or(6.0);
    let corner = ae_ceiling(&env, LimitKind::CornerCoverage).unwrap_or(diameter);
    let planned = env
        .stated
        .or(env.proposal.get().point())
        .expect("the 3D Rough states a point");
    let mut rng = Lcg(0x5EED_0017);
    let n_ramp = (SAMPLE_COUNT as f64 * SHARE_RAMP).round() as usize;
    let n_slot = (SAMPLE_COUNT as f64 * SHARE_SLOT).round() as usize;
    let n_corner = (SAMPLE_COUNT as f64 * SHARE_CORNER).round() as usize;
    let n_deep = (SAMPLE_COUNT as f64 * SHARE_DEEP).round() as usize;
    let samples: Vec<CutPoint3> = (0..SAMPLE_COUNT)
        .map(|i| {
            let (ae, ap) = if i < n_ramp {
                (diameter, planned.ap_mm * (0.1 + 0.9 * rng.next_unit()))
            } else if i < n_ramp + n_slot {
                (diameter, planned.ap_mm)
            } else if i < n_ramp + n_slot + n_corner {
                (
                    planned.ae_mm + (corner - planned.ae_mm).max(0.0) * rng.next_unit(),
                    planned.ap_mm,
                )
            } else if i < n_ramp + n_slot + n_corner + n_deep {
                (planned.ae_mm, planned.ap_mm * DEEP_FACTOR)
            } else {
                (
                    planned.ae_mm * (1.0 + JITTER * (2.0 * rng.next_unit() - 1.0)),
                    planned.ap_mm,
                )
            };
            CutPoint3 {
                ae_mm: ae,
                ap_mm: ap,
                force_n: None,
                weight_s: 1.0,
                move_index: i,
                sample_index: i,
            }
        })
        .collect();
    CutMetricSet {
        toolpath_id: pocket.toolpath_id,
        cards: pocket.cards.clone(),
        envelope: Some(std::sync::Arc::new(env)),
        samples,
        diameter_mm: diameter,
        budget_n: None,
        force_plane: None,
    }
}

/// P17b (a look-only set): the steady passes of the Pocket set spread
/// over [`SPREAD_CELLS`] heat cells each way in width and in depth, on a
/// triangular density that falls off from the planned point, so the heat
/// ramp shows every step. The explainer's prototype puts most of the
/// cutting time in the one cell under the planned dot, where no ramp can
/// show. No force on a sample (no dot), so the heat and the contours read
/// alone; the budget and the field stay, so the contours draw.
const SPREAD_CELLS: usize = 4;

fn spread_chart_set(pocket: &CutMetricSet) -> CutMetricSet {
    let env = pocket
        .envelope
        .as_deref()
        .expect("the Pocket set has its envelope");
    let scene = EnvelopeScene::of(env, None);
    let planned = env
        .stated
        .or(env.proposal.get().point())
        .expect("the fixture states or proposes a point");
    let diameter = pocket.diameter_mm;
    let mut rng = Lcg(0x5EED_0017);
    let width_cell = diameter * SPREAD_CELLS as f64 / GRID_COLUMNS as f64;
    let depth_cell = scene.ap_range.1 * SPREAD_CELLS as f64 / GRID_ROWS as f64;
    let samples: Vec<CutPoint3> = (0..SAMPLE_COUNT)
        .map(|i| {
            let tri = |rng: &mut Lcg| rng.next_unit() + rng.next_unit() - 1.0;
            let ae = (planned.ae_mm + tri(&mut rng) * width_cell).max(0.0);
            let ap = (planned.ap_mm + tri(&mut rng) * depth_cell).max(0.0);
            CutPoint3 {
                ae_mm: ae,
                ap_mm: ap,
                force_n: None,
                weight_s: 1.0,
                move_index: i,
                sample_index: i,
            }
        })
        .collect();
    CutMetricSet {
        samples,
        ..pocket.clone()
    }
}

/// Render `add` inside the app's inspector panel at `width` and save it.
fn snapshot_panel(
    dir: &Path,
    name: &str,
    width: f32,
    height: f32,
    add: impl Fn(&mut egui::Ui) + 'static,
) {
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(width + CENTRAL_WIDTH, height))
            .with_pixels_per_point(PIXELS_PER_POINT)
            .wgpu()
            .build_ui(move |ui| {
                side_panel(ui, SidePanelEdge::Right, "p17_sim_snapshot", width, |ui| {
                    add(ui);
                });
            })
    }));
    let mut harness = match built {
        Ok(harness) => harness,
        Err(err) => {
            let text = err
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "no message".to_owned());
            panic!("P17 blocker: no headless wgpu adapter: {text}");
        }
    };
    tokens::apply(&harness.ctx);
    tokens::apply_fonts(&harness.ctx);
    let _ = harness.run();
    let image = harness
        .render()
        .unwrap_or_else(|err| panic!("P17 blocker: the wgpu render failed: {err}"));
    let left = (CENTRAL_WIDTH * PIXELS_PER_POINT).round() as u32;
    let image =
        image::imageops::crop_imm(&image, left, 0, image.width() - left, image.height()).to_image();
    let path = dir.join(name);
    image
        .save(&path)
        .unwrap_or_else(|err| panic!("save {}: {err}", path.display()));
    eprintln!("P17 snapshot: {}", path.display());
}

#[test]
#[ignore = "writes PNG files and needs a wgpu adapter; set RS_CAM_P17_SNAPSHOT_DIR"]
fn the_sim_cards_and_chart_at_each_rail_width_p17() {
    let Some(dir) = snapshot_dir() else {
        eprintln!("RS_CAM_P17_SNAPSHOT_DIR is not set: no snapshot");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("mkdir {}: {err}", dir.display()));
    let set = std::sync::Arc::new(sim_set());
    let rough = std::sync::Arc::new(rough_chart_set(&set));
    for width in WIDTHS {
        let cards = std::sync::Arc::clone(&set);
        snapshot_panel(
            &dir,
            &format!("sim_cards_{width:.0}.png"),
            width,
            CARDS_HEIGHT,
            move |ui| {
                let _ = draw_cut_metric_cards(
                    ui,
                    CutMetricsBody {
                        set: &cards,
                        rows: &[],
                        over_time: &[],
                        playhead: None,
                        move_offset: 0,
                        regenerate_first: None,
                        extra_abstentions: Vec::new(),
                    },
                );
            },
        );
        let chart = std::sync::Arc::clone(&rough);
        snapshot_panel(
            &dir,
            &format!("sim_chart_{width:.0}.png"),
            width,
            CHART_HEIGHT,
            move |ui| {
                // The playhead in the steady passes, so the ring paints.
                let _ = draw_cut_chart_section(ui, &chart, Some(SAMPLE_COUNT / 2), SAMPLE_COUNT);
            },
        );
    }
    // The over-budget dots: the fixture cut sits under its budget, so a
    // what-if set with the budget at a fifth paints the dots and the
    // legend entry's hover. The cards are not part of this image.
    let mut what_if = (*set).clone();
    what_if.budget_n = what_if.budget_n.map(|b| b / 5.0);
    // P29-10 10g: the plane holds its own budget and level lines, so the
    // what-if moves them with the line of the dots.
    if let Some(plane) = what_if.force_plane.as_mut() {
        plane.budget_n /= 5.0;
        plane.levels = levels_at(&plane.field, plane.budget_n);
    }
    // P29-10 10g (ruling O4): the same cut with no force limit: the 100 %
    // line is faint and reads "budget, no limit here", and no dot paints.
    let mut no_limit = what_if.clone();
    no_limit.budget_n = None;
    if let Some(plane) = no_limit.force_plane.as_mut() {
        plane.role = BudgetRole::NoLimitHere;
    }
    snapshot_panel(
        &dir,
        "sim_chart_340_no_limit.png",
        340.0,
        CHART_HEIGHT,
        move |ui| {
            let _ = draw_cut_chart_section(ui, &no_limit, Some(SAMPLE_COUNT / 2), SAMPLE_COUNT);
        },
    );
    snapshot_panel(
        &dir,
        "sim_chart_340_over_budget.png",
        340.0,
        CHART_HEIGHT,
        move |ui| {
            let _ = draw_cut_chart_section(ui, &what_if, Some(SAMPLE_COUNT / 2), SAMPLE_COUNT);
        },
    );
    // P17b: the heat ramp on a spread set (look only).
    let spread = spread_chart_set(&set);
    snapshot_panel(
        &dir,
        "sim_chart_340_spread.png",
        340.0,
        CHART_HEIGHT,
        move |ui| {
            let _ = draw_cut_chart_section(ui, &spread, None, SAMPLE_COUNT);
        },
    );
}

// ── P17b item 4: the Passes tab feeds block (the envelope plot over the nomogram) ──

/// The height of the Passes screen: the inspector header, the tab strip,
/// the SAFE CUT plot and the nomogram under it (the op rows start below).
const PASSES_HEIGHT: f32 = 600.0;

/// The rivmap100 project on the Passes tab of the 3D Rough (the p5d
/// recipe): the memo fills on the first frames, so the nomogram draws.
fn rivmap_passes_state() -> rs_cam_viz::state::AppState {
    let session = rs_cam_core::session::ProjectSession::load(Path::new(FIXTURE))
        .expect("the rivmap100 fixture loads");
    let mut state = rs_cam_viz::state::AppState::new();
    state.replace_session(session);
    let id = state
        .session()
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .id;
    state
        .gui
        .toolpath_rt
        .insert(id, rs_cam_viz::state::runtime::ToolpathRuntime::new(true));
    let _ = state.set_focus(rs_cam_viz::state::selection::Selection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, rs_cam_viz::ui::properties::ToolpathTab::Passes));
    state
}

/// Render the whole inspector (`properties::draw`) on the Passes tab at
/// `width` and save it: the kit envelope plot over the kit nomogram.
fn snapshot_passes(dir: &Path, name: &str, width: f32) {
    let state = std::cell::RefCell::new(rivmap_passes_state());
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(width + CENTRAL_WIDTH, PASSES_HEIGHT))
            .with_pixels_per_point(PIXELS_PER_POINT)
            .wgpu()
            .build_ui(move |ui| {
                side_panel(
                    ui,
                    SidePanelEdge::Right,
                    "p17b_passes_snapshot",
                    width,
                    |ui| {
                        let mut events = Vec::new();
                        rs_cam_viz::ui::properties::draw(ui, &mut state.borrow_mut(), &mut events);
                    },
                );
            })
    }));
    let mut harness = match built {
        Ok(harness) => harness,
        Err(err) => {
            let text = err
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "no message".to_owned());
            panic!("P17 blocker: no headless wgpu adapter: {text}");
        }
    };
    tokens::apply(&harness.ctx);
    tokens::apply_fonts(&harness.ctx);
    // The memo and the label layout settle over a few frames.
    for _ in 0..6 {
        harness.step();
    }
    let image = harness
        .render()
        .unwrap_or_else(|err| panic!("P17 blocker: the wgpu render failed: {err}"));
    let left = (CENTRAL_WIDTH * PIXELS_PER_POINT).round() as u32;
    let image =
        image::imageops::crop_imm(&image, left, 0, image.width() - left, image.height()).to_image();
    let path = dir.join(name);
    image
        .save(&path)
        .unwrap_or_else(|err| panic!("save {}: {err}", path.display()));
    eprintln!("P17 snapshot: {}", path.display());
}

#[test]
#[ignore = "writes PNG files and needs a wgpu adapter; set RS_CAM_P17_SNAPSHOT_DIR"]
fn the_passes_tab_at_each_rail_width_p17b() {
    let Some(dir) = snapshot_dir() else {
        eprintln!("RS_CAM_P17_SNAPSHOT_DIR is not set: no snapshot");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("mkdir {}: {err}", dir.display()));
    for width in WIDTHS {
        snapshot_passes(&dir, &format!("passes_{width:.0}.png"), width);
    }
}

/// The level lines of `field` at the contour shares of `budget_n`.
fn levels_at(
    field: &rs_cam_core::tool_load::force_field::ForceField,
    budget_n: f64,
) -> Vec<ForceLevel> {
    CONTOUR_SHARES
        .into_iter()
        .map(|share| ForceLevel {
            share,
            force_n: share * budget_n,
            lines: contours(field, share * budget_n),
        })
        .filter(|level| !level.lines.is_empty())
        .collect()
}
