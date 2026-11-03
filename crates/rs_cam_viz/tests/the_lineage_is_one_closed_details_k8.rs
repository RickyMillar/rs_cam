//! K8 (feeds contract P5b, ruling Q7): the inspector shows the lineage of a
//! generated toolpath in ONE closed `Details`.
//!
//! # The claim
//!
//! - Closed, the Lineage is one row: one painted text, "Lineage · n",
//!   where n is the count of the open rows.
//! - Open, the rows match the core `Lineage` field by field: the cut
//!   (CutSpec), each stored speed with the `ProvKind` of its stamp (the one
//!   provenance vocabulary, decision D4), each tool fact with its bare
//!   value and no stress glyph (Q7), the entry and the findings.
//!
//! The record comes from a real generation (`generate_toolpath` of the
//! fixture Pocket), so a lineage field that generation does not fill turns
//! this red.
//!
//! Red before P5b: `ToolpathComputeResult::lineage` does not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::trace::lineage::{Lineage, axial_text, radial_text};
use rs_cam_viz::ui::components::ProvKind;
use rs_cam_viz::ui::properties::lineage::{LINEAGE_LABEL, draw_lineage, lineage_rows};

fn generated_pocket_lineage() -> Lineage {
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    let model = rs_cam_core::session::LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        path: std::path::PathBuf::from("lineage_fixture.svg"),
        name: "Lineage fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: std::sync::Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            rs_cam_core::polygon::Polygon2::rectangle(0.0, 0.0, 30.0, 20.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let stock = rs_cam_core::compute::stock_config::StockConfig {
        z: Some(10.0),
        ..Default::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(ToolConfig::new_default(
            ToolId::from_raw(1),
            ToolType::EndMill,
        ))
        .model(model);
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::ToolpathId::from_raw(0),
                name: "Pocket".to_owned(),
                enabled: true,
                operation: rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket),
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id: rs_cam_core::ids::ModelId::from_raw(0),
                pre_gcode: None,
                post_gcode: None,
                boundary: Default::default(),
                boundary_inherit: true,
                rest_analysis: Default::default(),
                stock_source: Default::default(),
                coolant: Default::default(),
                debug_options: Default::default(),
                feeds_provenance: Default::default(),
                planner_origin: None,
            },
        )
        .expect("add the Pocket");
    let mut session = builder.build();
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("the fixture Pocket generates");
    (*result.lineage).clone()
}

#[test]
fn the_lineage_is_one_closed_details_k8() {
    let lineage = generated_pocket_lineage();
    let rows = lineage_rows(&lineage);
    assert!(
        !lineage.tool_facts.is_empty(),
        "non-vacuity: generation records tool facts"
    );

    // Closed: one row, "Lineage · n".
    let ctx = egui::Context::default();
    rs_cam_viz::ui::tokens::apply(&ctx);
    rs_cam_viz::ui::tokens::apply_fonts(&ctx);
    let mut texts = Vec::new();
    for _ in 0..2 {
        texts.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_lineage(ui, rs_cam_core::ToolpathId::from_raw(0), &lineage);
        });
        for c in &out.shapes {
            if let egui::Shape::Text(t) = &c.shape {
                texts.push(t.galley.job.text.clone());
            }
        }
        out.textures_delta.clear();
    }
    assert_eq!(
        texts,
        vec![format!("{LINEAGE_LABEL} \u{00B7} {}", rows.len())],
        "closed, the Lineage is one row"
    );

    // Open: the rows match the core record field by field.
    let row = |name: &str| {
        rows.iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("no `{name}` row: {rows:?}"))
    };
    let (axial, axial_mm) = axial_text(lineage.cut.axial);
    let mut want = axial.to_owned();
    want[..1].make_ascii_uppercase();
    assert_eq!(
        row(&want).value,
        format!("{:.2} mm", axial_mm.expect("a Pocket states its depth"))
    );
    let (radial, radial_mm) = radial_text(lineage.cut.radial);
    let mut want = radial.to_owned();
    want[..1].make_ascii_uppercase();
    assert_eq!(
        row(&want).value,
        format!("{:.2} mm", radial_mm.expect("a Pocket states its stepover"))
    );
    for (name, unit, u) in [
        ("Spindle", "rpm", &lineage.speeds.spindle_rpm),
        ("Feed", "mm/min", &lineage.speeds.feed_rate),
        ("Plunge", "mm/min", &lineage.speeds.plunge_rate),
    ] {
        let Some(v) = u.value else { continue };
        let r = row(name);
        assert_eq!(r.value, format!("{v:.0} {unit}"), "{name}");
        assert_eq!(r.source, u.basis.as_ref().map(ProvKind::from), "{name}");
    }
    for f in &lineage.tool_facts {
        let r = row(f.fact.label());
        assert!(
            r.value.starts_with(&format!("{:.0}", f.value.trunc())),
            "{r:?}"
        );
        assert_eq!(r.source, None, "Q7: a tool fact takes no stress glyph");
    }
    assert!(row("Entry").hover.contains("Entry contract"));
    assert_eq!(
        rows.iter().any(|r| r.name == "Findings"),
        !lineage.findings.is_empty()
    );
}
