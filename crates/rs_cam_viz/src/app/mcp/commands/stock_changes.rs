//! S4 (`planning/stock_additions_2026-10-09/PLAN.md`): the four MCP
//! stock-change rows and the stock-change JSON that the read tools print.
//!
//! The wire tools are `add_stock_change`, `edit_stock_change`,
//! `move_stock_change` and `remove_stock_change`. Each turns into ONE S2
//! command through the core route of `commands.rs`; this file holds the
//! conversion and the reply. A refusal of the S2 validation reaches the
//! agent in the core's words (`Stock change refused: ...`).
//!
//! `list_setups` and `inspect_stock` print [`stock_change_json`], built
//! from `ProjectSession::stock_change_rows`: the same rows and the same
//! volume text as the GUI panel and the CLI.

use rs_cam_core::compute::stock_change::{
    CutAs, StockChange, StockChangeOp, StockGeometry, next_stock_change_id,
};
use rs_cam_core::ids::{ModelId, StockChangeId};
use rs_cam_core::material::Material;
use rs_cam_core::session::{
    AddStockChangeArgs, Command, Effects, MoveStockChangeArgs, RemoveStockChangeArgs,
    ReplaceStockChangeArgs, StockChangeRow,
};
use rs_cam_mcp::server::{
    AddStockChangeParam, CutAsParam, EditStockChangeParam, MoveStockChangeParam,
    RemoveStockChangeParam, StockChangeOpParam, StockGeometryKindParam, resolve_material,
};

use super::{CoreBefore, CorePlan, CoreReply};
use crate::app::RsCamApp;
use crate::mcp_bridge::mutation_error_json;

/// One stock-change row as the MCP read tools print it.
///
/// `volume` is `null` when the row has no measured volume, and
/// `volume_state` says why: `disabled` or `not_simulated`. `volume_label`
/// is the text the GUI row and the CLI line print.
pub(crate) fn stock_change_json(row: &StockChangeRow) -> serde_json::Value {
    let (volume, state) = match &row.volume {
        Ok(v) => (
            serde_json::json!({
                "added_mm3": v.added_mm3,
                "removed_mm3": v.removed_mm3,
                "added_ml": v.added_ml(),
                "removed_ml": v.removed_ml(),
                "net_ml": v.net_ml(),
            }),
            "measured",
        ),
        Err(rs_cam_core::session::StockChangeVolumeAbsence::Disabled) => {
            (serde_json::Value::Null, "disabled")
        }
        Err(rs_cam_core::session::StockChangeVolumeAbsence::NotSimulated) => {
            (serde_json::Value::Null, "not_simulated")
        }
    };
    let material = match row.change.op {
        StockChangeOp::Add => serde_json::json!(row.change.material.label()),
        StockChangeOp::Remove => serde_json::Value::Null,
    };
    serde_json::json!({
        "setup_index": row.setup_index,
        "setup_id": row.setup_id,
        "position": row.position,
        "id": row.change.id.0,
        "name": row.change.name,
        "enabled": row.change.enabled,
        "op": row.change.op.label(),
        "geometry": serde_json::to_value(&row.change.geometry)
            .unwrap_or(serde_json::Value::Null),
        "material": material,
        // S6: `stock_material` or `own_material` for an add; null for a
        // remove, which ignores it. `cut_as_label` is the GUI/CLI text.
        "cut_as": row.cut_as_label().map(|_| row.change.cut_as.token()),
        "cut_as_label": row.cut_as_label(),
        "volume": volume,
        "volume_state": state,
        "volume_label": row.volume_label(),
    })
}

fn cut_as_of(param: CutAsParam) -> CutAs {
    match param {
        CutAsParam::StockMaterial => CutAs::StockMaterial,
        CutAsParam::OwnMaterial => CutAs::OwnMaterial,
    }
}

fn op_of(param: StockChangeOpParam) -> StockChangeOp {
    match param {
        StockChangeOpParam::Add => StockChangeOp::Add,
        StockChangeOpParam::Remove => StockChangeOp::Remove,
    }
}

/// A wire refusal: the sentence and the field it names.
type WireRefusal = (String, &'static str);

/// Build a geometry from the wire's flat fields. Every field the kind needs
/// must be present, and a field the kind does not hold is refused.
fn geometry_of(
    kind: StockGeometryKindParam,
    model_ids: &[usize],
    level_z: Option<f64>,
    z_bottom: Option<f64>,
    z_top: Option<f64>,
) -> Result<StockGeometry, WireRefusal> {
    let ids: Vec<ModelId> = model_ids.iter().map(|&id| ModelId(id)).collect();
    let stray = |field: &'static str, kind: &str| -> WireRefusal {
        (
            format!("Error: {field} does not apply to kind '{kind}'"),
            field,
        )
    };
    let missing = |field: &'static str, kind: &str| -> WireRefusal {
        (format!("Error: kind '{kind}' needs {field}"), field)
    };
    match kind {
        StockGeometryKindParam::Model => {
            for (field, value) in [
                ("level_z", level_z),
                ("z_bottom", z_bottom),
                ("z_top", z_top),
            ] {
                if value.is_some() {
                    return Err(stray(field, "model"));
                }
            }
            let [model_id] = ids.as_slice() else {
                return Err((
                    format!(
                        "Error: kind 'model' takes exactly one model id, and the request gives {}",
                        ids.len()
                    ),
                    "model_ids",
                ));
            };
            Ok(StockGeometry::Model {
                model_id: *model_id,
            })
        }
        StockGeometryKindParam::OutlineFill => {
            for (field, value) in [("z_bottom", z_bottom), ("z_top", z_top)] {
                if value.is_some() {
                    return Err(stray(field, "outline_fill"));
                }
            }
            Ok(StockGeometry::OutlineFill {
                model_ids: ids,
                level_z: level_z.ok_or_else(|| missing("level_z", "outline_fill"))?,
            })
        }
        StockGeometryKindParam::OutlineExtrude => {
            if level_z.is_some() {
                return Err(stray("level_z", "outline_extrude"));
            }
            Ok(StockGeometry::OutlineExtrude {
                model_ids: ids,
                z_bottom: z_bottom.ok_or_else(|| missing("z_bottom", "outline_extrude"))?,
                z_top: z_top.ok_or_else(|| missing("z_top", "outline_extrude"))?,
            })
        }
    }
}

/// Patch the current geometry with the fields the request gives, without a
/// kind change.
fn patch_geometry(
    current: &StockGeometry,
    model_ids: Option<&[usize]>,
    level_z: Option<f64>,
    z_bottom: Option<f64>,
    z_top: Option<f64>,
) -> Result<StockGeometry, WireRefusal> {
    match current {
        StockGeometry::Model { model_id } => {
            let ids = model_ids.map_or_else(|| vec![model_id.0], <[usize]>::to_vec);
            geometry_of(
                StockGeometryKindParam::Model,
                &ids,
                level_z,
                z_bottom,
                z_top,
            )
        }
        StockGeometry::OutlineFill {
            model_ids: ids,
            level_z: level,
        } => {
            let ids =
                model_ids.map_or_else(|| ids.iter().map(|id| id.0).collect(), <[usize]>::to_vec);
            geometry_of(
                StockGeometryKindParam::OutlineFill,
                &ids,
                Some(level_z.unwrap_or(*level)),
                z_bottom,
                z_top,
            )
        }
        StockGeometry::OutlineExtrude {
            model_ids: ids,
            z_bottom: bottom,
            z_top: top,
        } => {
            let ids =
                model_ids.map_or_else(|| ids.iter().map(|id| id.0).collect(), <[usize]>::to_vec);
            geometry_of(
                StockGeometryKindParam::OutlineExtrude,
                &ids,
                level_z,
                Some(z_bottom.unwrap_or(*bottom)),
                Some(z_top.unwrap_or(*top)),
            )
        }
    }
}

/// The material from the wire: a catalogue name, a custom name, or the
/// default.
fn material_of(
    material: Option<&str>,
    custom: Option<&str>,
    default: &Material,
) -> Result<Material, WireRefusal> {
    match (material, custom) {
        (Some(_), Some(_)) => Err((
            "Error: give material or custom_material, not both".to_owned(),
            "custom_material",
        )),
        (Some(name), None) => {
            resolve_material(name).map_err(|e| (format!("Error: {e}"), "material"))
        }
        (None, Some(name)) => {
            let name = name.trim();
            if name.is_empty() {
                return Err((
                    "Error: custom_material must be a non-empty name".to_owned(),
                    "custom_material",
                ));
            }
            Ok(Material::Custom {
                name: name.to_owned(),
                feed_scale_factor: 1.0,
            })
        }
        (None, None) => Ok(default.clone()),
    }
}

fn refuse((message, field): WireRefusal) -> CorePlan {
    CorePlan::Answered(mutation_error_json(&message, Some(field)))
}

impl RsCamApp {
    /// The stock changes of the setup at `setup_index`, or the refusal
    /// reply that names the index.
    fn stock_change_setup(&self, setup_index: usize) -> Result<Vec<StockChange>, String> {
        self.controller
            .state()
            .session
            .list_setups()
            .get(setup_index)
            .map(|setup| setup.stock_changes.clone())
            .ok_or_else(|| {
                mutation_error_json(
                    &format!("Error: Setup index {setup_index} not found"),
                    Some("setup_index"),
                )
            })
    }

    /// `add_stock_change` → `Command::AddStockChange`.
    pub(super) fn plan_add_stock_change(
        &self,
        p: &AddStockChangeParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let stock_changes = match self.stock_change_setup(p.setup_index) {
            Ok(changes) => changes,
            Err(reply) => return CorePlan::Answered(reply),
        };
        let geometry = match geometry_of(p.kind, &p.model_ids, p.level_z, p.z_bottom, p.z_top) {
            Ok(geometry) => geometry,
            Err(refusal) => return refuse(refusal),
        };
        let stock_material = self
            .controller
            .state()
            .session
            .stock_config()
            .material
            .clone();
        let material = match material_of(
            p.material.as_deref(),
            p.custom_material.as_deref(),
            &stock_material,
        ) {
            Ok(material) => material,
            Err(refusal) => return refuse(refusal),
        };
        let id = next_stock_change_id(&stock_changes);
        let change = StockChange {
            id,
            name: p
                .name
                .clone()
                .unwrap_or_else(|| format!("Change {}", id.0 + 1)),
            enabled: p.enabled.unwrap_or(true),
            op: op_of(p.op),
            geometry,
            material,
            display_colour: None,
            cut_as: cut_as_of(p.cut_as),
        };
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({ "change_id": id.0 });
        CorePlan::Apply(
            Command::AddStockChange(AddStockChangeArgs {
                setup_index: p.setup_index,
                change: Box::new(change),
            }),
            Box::new(before),
        )
    }

    /// `edit_stock_change` → `Command::ReplaceStockChange` with the stored
    /// record patched by the given fields.
    pub(super) fn plan_edit_stock_change(
        &self,
        p: &EditStockChangeParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let stock_changes = match self.stock_change_setup(p.setup_index) {
            Ok(changes) => changes,
            Err(reply) => return CorePlan::Answered(reply),
        };
        let change_id = StockChangeId(p.change_id);
        let Some(stored) = stock_changes.iter().find(|c| c.id == change_id) else {
            return CorePlan::Answered(mutation_error_json(
                &format!(
                    "Error: setup {} carries no stock change with id {}",
                    p.setup_index, p.change_id
                ),
                Some("change_id"),
            ));
        };
        let mut change = stored.clone();
        if let Some(name) = &p.name {
            change.name.clone_from(name);
        }
        if let Some(enabled) = p.enabled {
            change.enabled = enabled;
        }
        if let Some(op) = p.op {
            change.op = op_of(op);
        }
        let geometry = match p.kind {
            Some(kind) => {
                let ids = p
                    .model_ids
                    .clone()
                    .unwrap_or_else(|| change.geometry.model_ids().iter().map(|id| id.0).collect());
                geometry_of(kind, &ids, p.level_z, p.z_bottom, p.z_top)
            }
            None => patch_geometry(
                &change.geometry,
                p.model_ids.as_deref(),
                p.level_z,
                p.z_bottom,
                p.z_top,
            ),
        };
        change.geometry = match geometry {
            Ok(geometry) => geometry,
            Err(refusal) => return refuse(refusal),
        };
        change.material = match material_of(
            p.material.as_deref(),
            p.custom_material.as_deref(),
            &change.material,
        ) {
            Ok(material) => material,
            Err(refusal) => return refuse(refusal),
        };
        if let Some(cut_as) = p.cut_as {
            change.cut_as = cut_as_of(cut_as);
        }
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({ "change_id": p.change_id });
        CorePlan::Apply(
            Command::ReplaceStockChange(ReplaceStockChangeArgs {
                setup_index: p.setup_index,
                change_id,
                change: Box::new(change),
            }),
            Box::new(before),
        )
    }

    /// `move_stock_change` → `Command::MoveStockChange`. Core refuses an
    /// unknown id and a position out of range.
    pub(super) fn plan_move_stock_change(
        &self,
        p: &MoveStockChangeParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        if let Err(reply) = self.stock_change_setup(p.setup_index) {
            return CorePlan::Answered(reply);
        }
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({
            "change_id": p.change_id,
            "to_position": p.to_position,
        });
        CorePlan::Apply(
            Command::MoveStockChange(MoveStockChangeArgs {
                setup_index: p.setup_index,
                change_id: StockChangeId(p.change_id),
                to_position: p.to_position,
            }),
            Box::new(before),
        )
    }

    /// `remove_stock_change` → `Command::RemoveStockChange`. Core refuses
    /// an unknown id.
    pub(super) fn plan_remove_stock_change(
        &self,
        p: &RemoveStockChangeParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        if let Err(reply) = self.stock_change_setup(p.setup_index) {
            return CorePlan::Answered(reply);
        }
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({ "change_id": p.change_id });
        CorePlan::Apply(
            Command::RemoveStockChange(RemoveStockChangeArgs {
                setup_index: p.setup_index,
                change_id: StockChangeId(p.change_id),
            }),
            Box::new(before),
        )
    }

    /// The reply of an applied stock-change row: the setup's list after
    /// the write, and the stale set.
    pub(super) fn describe_stock_change(
        &mut self,
        verb: &str,
        effects: &Effects,
        before: &CoreBefore,
    ) -> CoreReply {
        self.controller.state_mut().gui.mark_edited();
        if effects.simulation_cleared {
            self.controller.invalidate_simulation();
        }
        let setup_index = before.index();
        let change_id = before
            .extra
            .get("change_id")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        // A GUI editor open on a change the wire removed would apply to
        // nothing; close it.
        {
            let state = self.controller.state_mut();
            let removed = verb == "Removed";
            if removed
                && state.panels.stock_change_editor.as_ref().is_some_and(|e| {
                    !e.is_new && u64::try_from(e.draft.id.0).is_ok_and(|id| id == change_id)
                })
            {
                state.panels.stock_change_editor = None;
            }
        }
        let stale = self.core_stale(effects);
        let rows: Vec<serde_json::Value> = self
            .controller
            .state()
            .session
            .stock_change_rows()
            .iter()
            .filter(|row| row.setup_index == setup_index)
            .map(stock_change_json)
            .collect();
        CoreReply::quiet(self.mcp_mutation_result(
            format!("{verb} stock change {change_id} in setup {setup_index}"),
            serde_json::json!({
                "setup_index": setup_index,
                "change_id": change_id,
                "simulation_cleared": effects.simulation_cleared,
                "stock_changes": rows,
            }),
            stale,
            &before.diagnostics,
        ))
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed conversion is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_needs_its_own_fields_and_refuses_the_others() {
        assert_eq!(
            geometry_of(
                StockGeometryKindParam::OutlineFill,
                &[3],
                Some(2.0),
                None,
                None
            ),
            Ok(StockGeometry::OutlineFill {
                model_ids: vec![ModelId(3)],
                level_z: 2.0
            })
        );
        assert_eq!(
            geometry_of(StockGeometryKindParam::OutlineFill, &[3], None, None, None)
                .unwrap_err()
                .1,
            "level_z"
        );
        assert_eq!(
            geometry_of(
                StockGeometryKindParam::OutlineFill,
                &[3],
                Some(2.0),
                Some(0.0),
                None
            )
            .unwrap_err()
            .1,
            "z_bottom"
        );
        assert_eq!(
            geometry_of(StockGeometryKindParam::Model, &[1, 2], None, None, None)
                .unwrap_err()
                .1,
            "model_ids"
        );
        assert_eq!(
            geometry_of(
                StockGeometryKindParam::OutlineExtrude,
                &[3],
                None,
                Some(-1.0),
                Some(4.0)
            ),
            Ok(StockGeometry::OutlineExtrude {
                model_ids: vec![ModelId(3)],
                z_bottom: -1.0,
                z_top: 4.0
            })
        );
    }

    #[test]
    fn a_patch_keeps_what_the_request_does_not_give() {
        let current = StockGeometry::OutlineExtrude {
            model_ids: vec![ModelId(3)],
            z_bottom: -1.0,
            z_top: 4.0,
        };
        assert_eq!(
            patch_geometry(&current, None, None, None, Some(6.0)),
            Ok(StockGeometry::OutlineExtrude {
                model_ids: vec![ModelId(3)],
                z_bottom: -1.0,
                z_top: 6.0
            })
        );
        assert_eq!(
            patch_geometry(&current, None, Some(1.0), None, None)
                .unwrap_err()
                .1,
            "level_z",
            "a level does not apply to an extrude"
        );
    }

    #[test]
    fn the_material_is_a_catalogue_name_a_custom_name_or_the_default() {
        let default = Material::default();
        assert_eq!(material_of(None, None, &default), Ok(default.clone()));
        assert_eq!(
            material_of(None, Some(" Resin "), &default),
            Ok(Material::Custom {
                name: "Resin".to_owned(),
                feed_scale_factor: 1.0
            })
        );
        assert_eq!(
            material_of(Some("White Oak"), Some("Resin"), &default)
                .unwrap_err()
                .1,
            "custom_material"
        );
        assert_eq!(
            material_of(Some("no such material at all"), None, &default)
                .unwrap_err()
                .1,
            "material"
        );
    }

    // ── S4 parity: the GUI row and the MCP reply print the core volume ──

    fn parity_fixture() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/stock_change_parity_s4/project.toml")
    }

    /// Every text the setup inspector paints, on the second of two frames.
    fn painted_setup_panel(state: &mut crate::state::AppState) -> Vec<String> {
        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut texts = Vec::new();
        for pass in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::Panel::right("stock_change_parity")
                    .default_size(320.0)
                    .show(ui, |ui| {
                        let mut events = Vec::new();
                        crate::ui::properties::draw(ui, state, &mut events);
                    });
            });
            if pass == 1 {
                for clipped in &out.shapes {
                    if let egui::epaint::Shape::Text(text) = &clipped.shape {
                        texts.push(text.galley.job.text.clone());
                    }
                }
            }
            out.textures_delta.clear();
        }
        texts
    }

    /// GUI/MCP/CLI number parity (S4), the GUI and MCP half. The CLI half
    /// is `rs_cam_cli/tests/stock_change_lines_s4.rs`, on the same fixture.
    /// All three print `ProjectSession::stock_change_rows`.
    #[test]
    fn the_gui_row_and_the_mcp_reply_print_the_core_volume_s4() {
        use std::sync::atomic::AtomicBool;

        let mut session = rs_cam_core::session::ProjectSession::load(&parity_fixture())
            .expect("the fixture loads");
        let opts = rs_cam_core::session::SimulationOptions {
            resolution: session.simulation_resolution_mm(),
            ..rs_cam_core::session::SimulationOptions::default()
        };
        let _ = session
            .run_simulation(&opts, &AtomicBool::new(false))
            .expect("the stock changes apply");
        let rows = session.stock_change_rows();
        assert_eq!(rows.len(), 2, "the fixture holds two stock changes");
        for row in &rows {
            let measured = row.volume.as_ref().expect("each change is measured");
            assert!(measured.net_ml().abs() > 0.1, "non-vacuity: {measured:?}");
        }

        // MCP: the reply carries the core label and the core numbers.
        for row in &rows {
            let json = stock_change_json(row);
            let measured = row.volume.as_ref().unwrap();
            assert_eq!(json["volume_label"], serde_json::json!(row.volume_label()));
            assert_eq!(
                json["volume"]["net_ml"],
                serde_json::json!(measured.net_ml())
            );
            assert_eq!(json["volume_state"], serde_json::json!("measured"));
        }

        // GUI: the setup panel paints the same label for each row.
        let setup_id = rows[0].setup_id;
        let mut state = crate::state::AppState::new();
        state.session = session;
        state.selection =
            crate::state::selection::Selection::Setup(crate::state::job::SetupId(setup_id));
        let texts = painted_setup_panel(&mut state);
        assert!(
            texts.iter().any(|t| t == "Stock changes"),
            "the section is drawn: {texts:?}"
        );
        for row in &rows {
            assert!(
                texts.iter().any(|t| *t == row.volume_label()),
                "the GUI row paints `{}`: {texts:?}",
                row.volume_label()
            );
        }
    }

    // ── S6 parity: the "cut as" field on the GUI, MCP and CLI ──

    /// GUI/MCP/CLI parity of `cut_as` (S6), the GUI and MCP half. The CLI
    /// half is `rs_cam_cli/tests/stock_change_cut_as_s6.rs`, on the same
    /// fixture. All three read `StockChangeRow::cut_as_label`.
    #[test]
    fn the_gui_and_the_mcp_reply_name_the_cut_as_s6() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/stock_change_cut_as_s6/project.toml");
        let session =
            rs_cam_core::session::ProjectSession::load(&fixture).expect("the fixture loads");
        let rows = session.stock_change_rows();
        assert_eq!(rows.len(), 2);
        let (slab, pour) = (&rows[0], &rows[1]);
        assert_eq!(pour.change.cut_as, CutAs::OwnMaterial);
        assert_eq!(pour.cut_as_label(), Some("own material"));
        assert_eq!(slab.cut_as_label(), None, "a remove ignores cut_as");

        // MCP: the token and the label.
        let json = stock_change_json(pour);
        assert_eq!(json["cut_as"], serde_json::json!("own_material"));
        assert_eq!(json["cut_as_label"], serde_json::json!("own material"));
        let json = stock_change_json(slab);
        assert!(json["cut_as"].is_null() && json["cut_as_label"].is_null());

        // MCP write: the wire token reaches the record; absent is the default.
        let add: AddStockChangeParam = serde_json::from_value(serde_json::json!({
            "setup_index": 0, "kind": "outline_fill", "model_ids": [0],
            "level_z": 0.0, "cut_as": "own_material"
        }))
        .unwrap();
        assert_eq!(cut_as_of(add.cut_as), CutAs::OwnMaterial);
        let add: AddStockChangeParam = serde_json::from_value(serde_json::json!({
            "setup_index": 0, "kind": "outline_fill", "model_ids": [0], "level_z": 0.0
        }))
        .unwrap();
        assert_eq!(cut_as_of(add.cut_as), CutAs::StockMaterial);

        // GUI: the row paints the same label.
        let setup_id = pour.setup_id;
        let summary = crate::ui::properties::stock_changes::row_summary(pour);
        assert!(summary.ends_with("(own material)"), "{summary}");
        let mut state = crate::state::AppState::new();
        state.session = session;
        state.selection =
            crate::state::selection::Selection::Setup(crate::state::job::SetupId(setup_id));
        let texts = painted_setup_panel(&mut state);
        assert!(
            texts.contains(&summary),
            "the GUI row paints `{summary}`: {texts:?}"
        );
    }
}
