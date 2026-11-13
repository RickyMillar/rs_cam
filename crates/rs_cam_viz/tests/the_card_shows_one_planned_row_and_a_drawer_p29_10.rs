//! **P29-10 (10f): the Suggest card shows one limit row and a drawer.**
//!
//! # The rulings
//!
//! `P29-10_DESIGN.md` 3.6 and the lead rulings O1, O2 and O6 (section 12):
//! the face of the Suggest card on the Passes tab takes at most ONE limit
//! row: the worst row by the one core rank (`CheckRow::rank_cmp`), and only
//! when it exceeds. Every row sits in one closed drawer, "Limits · n", in
//! two parts: "As the op stands" (the stated rows) and "If you apply ⚡"
//! (the planned rows at the proposal, then the RPM rows). The rows are the
//! rows of `ProjectSession::limit_rows`, so the card, MCP and the CLI show
//! the same rows.
//!
//! # The claims
//!
//! For each op of a fixture (a 3D rough, a Scallop finish and a 2D
//! Adaptive with a ramp entry, in hardwood):
//!
//! 1. The memo rows are the session door's rows: `SafeEnvelope::rows` of the
//!    memo envelope equal `limit_rows(..).envelope()`, and
//!    `FeedsMemo::rpm_rows` equal `limit_rows(..).rpm()`.
//! 2. With the drawer closed, the panel paints at most one row face: the
//!    face of the worst STATED row by the core rank, when it exceeds (lead
//!    ruling 1 on 10f: the face describes the op as it stands; a proposal
//!    row never reaches it). The drawer header "Limits · n" paints, with
//!    " · k over" when k rows exceed.
//! 3. With the drawer open and every fold open, every row paints its core
//!    face without its subject (`CheckRow::face_in_group`), no such row is
//!    elided at the rail width, the part-two title ends with " · k over"
//!    exactly when k proposal rows exceed, and part one comes first.
//! 4. The ⚡ hover reads the binding planned row
//!    (`envelope::binding_planned_row`: the planned row of the binding limit
//!    on the declared cut that sets it), and `envelope::proposal_hover`
//!    names its face. The `OutsideEnvelope` line is held by the lib test
//!    `the_outside_line_reads_the_stated_row_p29_10` (the card's Suggest run
//!    raises it only on an apply with a pick).
//!
//! The fixture runs Constant chip, so the RPM rows are not empty. The four
//! operator files are not read (F155).
//!
//! Non-vacuity: an op has a face row, an op has both parts, an op has a
//! binding limit on a declared cut, and an op has RPM rows.
//!
//! Red before 10f: the card reads no `CheckRow` (claims 2 and 3 fail; the
//! memo has no `rpm_rows`, so the file does not compile).
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_card_shows_one_planned_row_and_a_drawer_p29_10
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::limits::CheckRow;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::properties::ToolpathTab;
use rs_cam_viz::ui::{properties, tokens};

const PANEL_WIDTH: f32 = 340.0;
const END_MILL: ToolId = ToolId::from_raw(1);
const BALL: ToolId = ToolId::from_raw(2);

fn tool(id: ToolId, kind: ToolType) -> ToolConfig {
    let mut tool = ToolConfig::test_default(id, kind);
    tool.diameter = 6.0;
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn config(name: &str, tool_id: ToolId, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::placeholder(),
        name: name.to_owned(),
        enabled: true,
        dressups: DressupConfig::for_op(operation.op_type()),
        operation,
        heights: Default::default(),
        tool_id: Some(tool_id),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

/// A rough, a finish after it and a 2D Adaptive with a ramp entry, in a
/// hardwood (a material with a force line, so the force budget judges).
fn fixture() -> AppState {
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("p29_10f_fixture.svg"),
        name: "P29-10f fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };
    let mut rough = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Adaptive3d);
    if let OperationConfig::Adaptive3d(c) = &mut rough {
        c.stock_to_leave_axial = 0.5;
        // A deep stated depth, so a limit of a declared cut lowers the
        // proposal (the ⚡ hover then names its planned row).
        c.depth_per_pass = Some(10.0);
        c.stepover = Some(5.0);
    }
    let finish = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Scallop);
    let mut ramp = config(
        "a ramp entry",
        END_MILL,
        rs_cam_core::compute::fixture_values::fixture_op(OperationType::Adaptive),
    );
    ramp.dressups.entry_style = DressupEntryStyle::Ramp;
    // Constant chip, so the Suggest RPM has its limit rows (the card shows
    // them in part two; claim 1 compares them with the session door).
    let post = rs_cam_core::gcode::PostConfig {
        spindle_strategy: rs_cam_core::feeds::SpindleStrategyKind::ConstantChip,
        spindle_rpm_ceiling: Some(24_000),
        ..Default::default()
    };
    // Constant chip needs the axis rates and accelerations of the machine
    // (the Shapeoko VFD preset, the axis values of the p23b fixture).
    let machine = rs_cam_core::machine::MachineProfile {
        max_feed_mm_min: 15_000.0,
        ..rs_cam_core::machine::MachineProfile::shapeoko_vfd()
    }
    .with_axis_limits([15_000.0, 15_000.0, 2500.0], [1200.0, 1200.0, 550.0]);
    let mut builder = ProjectSessionBuilder::new()
        .machine(machine)
        .post(post)
        .stock(stock)
        .tool(tool(END_MILL, ToolType::EndMill))
        .tool(tool(BALL, ToolType::BallNose))
        .model(model);
    for cfg in [
        config("the rough", END_MILL, rough),
        config("the finish", BALL, finish),
        ramp,
    ] {
        builder
            .add_toolpath(0, cfg)
            .expect("add a fixture toolpath");
    }
    let mut state = AppState::new();
    state.replace_session(builder.build());
    state
}

/// One painted single-line text: the text and whether egui elided it.
struct Painted {
    text: String,
    elided: bool,
}

/// Draw the real inspector on the Passes tab of toolpath `id` and return
/// the single-line texts of the last frame, in paint order. `visible`
/// opens every closed disclosure (the folds of the drawer, "Recommended").
fn draw(
    state: &mut AppState,
    id: rs_cam_core::ToolpathId,
    drawer_open: bool,
    visible: bool,
) -> Vec<Painted> {
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    let _ = state.set_focus(Selection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, ToolpathTab::Passes));
    state.panels.card_limits_open = drawer_open;
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    if visible {
        ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    }
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 20000.0),
        )),
        ..Default::default()
    };
    let mut texts = Vec::new();
    for pass in 0..3 {
        let mut out = ctx.run_ui(input(), |ui| {
            egui::Panel::right("p29_10f_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
                    });
                });
        });
        if pass == 2 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape
                    && !text.galley.job.text.contains('\n')
                {
                    texts.push(Painted {
                        text: text.galley.job.text.clone(),
                        elided: text.galley.elided,
                    });
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

/// The face of a painted row: a row with a hover paints its name and the
/// detail mark; the face row of the card paints the role glyph first.
fn painted_face(text: &str) -> &str {
    let text = text
        .strip_suffix(tokens::GLYPH_DETAIL)
        .map_or(text, str::trim_end);
    [tokens::GLYPH_DANGER, tokens::GLYPH_CAUTION]
        .iter()
        .find_map(|glyph| text.strip_prefix(glyph))
        .map_or(text, str::trim_start)
}

#[test]
fn the_card_shows_one_planned_row_and_a_drawer_p29_10() {
    let mut state = fixture();
    let ids: Vec<_> = state
        .session()
        .toolpath_configs()
        .iter()
        .map(|tc| (tc.id, tc.name.clone()))
        .collect();
    let mut ops_with_a_face_row = 0;
    let mut ops_with_both_parts = 0;
    let mut ops_with_a_binding_row = 0;
    let mut ops_with_rpm_rows = 0;
    for (index, (id, name)) in ids.into_iter().enumerate() {
        // Claim 2 (closed): drawn first, so the memo is full for claim 1.
        let closed = draw(&mut state, id, false, false);
        let session = state.session();
        let door = session
            .limit_rows(index, None, rs_cam_viz::state::simulation::run_modulation())
            .expect("the toolpath exists");
        let envelope = door
            .envelope()
            .expect("the op has a tool")
            .expect("the session envelope builds");
        let rpm = door.rpm();
        let memo = &state
            .gui
            .toolpath_rt
            .get(&id)
            .expect("the drawn toolpath has a runtime")
            .feeds_memo;

        // 1. The memo rows are the session door's rows.
        let memo_env = memo.envelope().expect("the memo holds the envelope");
        assert_eq!(
            format!("{:?}", memo_env.rows()),
            format!("{envelope:?}"),
            "{name}: the card envelope rows equal `limit_rows(..).envelope()`"
        );
        assert_eq!(
            format!("{:?}", memo.rpm_rows()),
            format!("{rpm:?}"),
            "{name}: the card RPM rows equal `limit_rows(..).rpm()`"
        );
        if !rpm.is_empty() {
            ops_with_rpm_rows += 1;
        }

        // 4. The ⚡ hover reads the binding planned row.
        if let Some(kind) = memo_env.proposal_binding
            && let Some(cut) = memo_env.limit(kind).and_then(|l| l.binding_cut)
        {
            ops_with_a_binding_row += 1;
            let row = rs_cam_viz::ui::feeds::envelope::binding_planned_row(memo_env)
                .unwrap_or_else(|| panic!("{name}: the binding limit has its planned row"));
            assert_eq!(row.cut(), Some(cut), "{name}: the row of the binding cut");
            assert_eq!(
                row.check().limit,
                rs_cam_core::limits::LimitId::of_limit_kind(kind),
                "{name}: the row of the binding limit"
            );
            let hover = rs_cam_viz::ui::feeds::envelope::proposal_hover(memo_env);
            assert_eq!(
                hover,
                format!(
                    "Apply the proposal and its speeds. The proposal stands on: {}.",
                    row.face()
                ),
                "{name}: the ⚡ hover names the binding planned row"
            );
        }

        let stated: Vec<&CheckRow> = envelope.iter().filter(|row| row.is_stated()).collect();
        let applied: Vec<&CheckRow> = envelope
            .iter()
            .filter(|row| row.is_planned())
            .chain(rpm.iter())
            .collect();
        let all: Vec<&CheckRow> = stated.iter().chain(applied.iter()).copied().collect();
        let faces: BTreeSet<&str> = all.iter().map(|row| row.face()).collect();
        // The face row describes the op as it stands (lead ruling 1 on
        // 10f): the worst STATED row by the rank, only when it exceeds.
        // P29-10z: every stated row ranks.
        let expected = stated
            .iter()
            .copied()
            .filter(|row| row.tone().exceeds())
            .min_by(|a, b| a.rank_cmp(b));
        let over = all.iter().filter(|row| row.tone().exceeds()).count();
        let applied_over = applied.iter().filter(|row| row.tone().exceeds()).count();

        // 2. Closed: one face row or none, never a proposal row, and the
        //    drawer header.
        let painted: Vec<&str> = closed
            .iter()
            .map(|p| painted_face(&p.text))
            .filter(|t| faces.contains(t))
            .collect();
        match expected {
            Some(row) => {
                ops_with_a_face_row += 1;
                assert_eq!(
                    painted,
                    vec![row.face()],
                    "{name}: the closed card paints ONE limit row, the worst stated row"
                );
            }
            None => assert!(
                painted.is_empty(),
                "{name}: no stated row exceeds, so the face paints no limit row: {painted:?}"
            ),
        }
        let header = if over > 0 {
            format!("Limits \u{00B7} {} \u{00B7} {over} over", all.len())
        } else {
            format!("Limits \u{00B7} {}", all.len())
        };
        assert!(
            closed.iter().any(|p| p.text == header),
            "{name}: the drawer header {header:?} paints"
        );

        // 3. Open, every fold open: every row paints its face without its
        //    subject, none is elided, the part-two title counts its rows
        //    that exceed, and the stated rows come first.
        let open = draw(&mut state, id, true, true);
        let group_faces: BTreeSet<&str> = all.iter().map(|row| row.face_in_group()).collect();
        let painted: BTreeSet<&str> = open.iter().map(|p| painted_face(&p.text)).collect();
        for face in &group_faces {
            assert!(
                painted.contains(face),
                "{name}: the open drawer paints the row {face:?}"
            );
        }
        for p in &open {
            if group_faces.contains(painted_face(&p.text)) {
                assert!(
                    !p.elided,
                    "{name}: the drawer row {:?} is clipped at {PANEL_WIDTH} points",
                    p.text
                );
            }
        }
        let first = open
            .iter()
            .position(|p| p.text.eq_ignore_ascii_case("as the op stands"));
        let second = open
            .iter()
            .position(|p| p.text.to_lowercase().starts_with("if you apply"));
        if let Some(title) = second.and_then(|k| open.get(k)) {
            let title = title.text.to_lowercase();
            assert_eq!(
                title.ends_with(&format!("{applied_over} over")),
                applied_over > 0,
                "{name}: the part-two title {title:?} counts its {applied_over} rows that exceed"
            );
        }
        if !stated.is_empty() && envelope.iter().any(CheckRow::is_planned) {
            ops_with_both_parts += 1;
            let first = first.unwrap_or_else(|| panic!("{name}: the first title paints"));
            let second = second.unwrap_or_else(|| panic!("{name}: the second title paints"));
            assert!(first < second, "{name}: the stated part comes first");
        }
    }
    assert!(ops_with_a_face_row > 0, "non-vacuity: an op has a face row");
    assert!(
        ops_with_both_parts > 0,
        "non-vacuity: an op has a stated part and a planned part"
    );
    assert!(
        ops_with_a_binding_row > 0,
        "non-vacuity: an op has a binding limit on a declared cut"
    );
    assert!(ops_with_rpm_rows > 0, "non-vacuity: an op has RPM rows");
}
