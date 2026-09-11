//! Save a [`ProjectSession`] back to a TOML project file.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use tracing::instrument;

use super::project_file::{
    ProjectFile, ProjectFixtureSection, ProjectJobSection, ProjectKeepOutSection,
    ProjectModelSection, ProjectPostConfig, ProjectSetupSection, ProjectStockConfig,
    ProjectToolSection, ProjectToolpathSection,
};
use super::{ProjectSession, SessionError};
use crate::compute::tool_config::{BitCutDirection, ToolMaterial, ToolType};

/// Convert a `ToolType` to the string key used in project files.
fn tool_type_to_key(tt: ToolType) -> String {
    match tt {
        ToolType::EndMill => "end_mill",
        ToolType::BallNose => "ball_nose",
        ToolType::BullNose => "bull_nose",
        ToolType::VBit => "v_bit",
        ToolType::TaperedBallNose => "tapered_ball_nose",
    }
    .to_owned()
}

/// Convert a `ToolMaterial` to the string key used in project files.
fn tool_material_to_key(tm: ToolMaterial) -> String {
    match tm {
        ToolMaterial::Carbide => "carbide",
        ToolMaterial::Hss => "hss",
    }
    .to_owned()
}

/// Convert a `BitCutDirection` to the string key used in project files.
fn cut_direction_to_key(cd: BitCutDirection) -> String {
    match cd {
        BitCutDirection::UpCut => "up_cut",
        BitCutDirection::DownCut => "down_cut",
        BitCutDirection::Compression => "compression",
    }
    .to_owned()
}

/// How a model path is written to a project file (G-MODELRELINK, F4.3).
///
/// **Relative when the model sits under the project directory, absolute
/// otherwise.** A project folder that holds its own models can be copied to
/// another machine, or moved on this one, and still open — which is the case
/// this exists for. A model outside that folder has no shorter honest
/// description than its absolute path; writing `../../..` chains instead
/// would be fragile, unreadable, and undefined across Windows drives.
///
/// The save used to be a pass-through of whatever
/// [`crate::session::LoadedModel::path`] happened to hold, so a project was
/// absolute-or-relative according to how its models were first added. Since
/// F4.3 the in-memory path is always the resolved absolute one
/// (`project_file::resolve_model_path`), so this is the single place the
/// question is answered.
///
/// Ported verbatim in behaviour from `rs_cam_viz`'s fallback loader
/// (`io/project.rs::persist_model_path`), which had the rule right and no
/// production caller. That copy stays where it is; it belongs to that
/// loader.
fn persist_model_path(project_dir: Option<&Path>, model_path: &Path) -> String {
    let Some(project_dir) = project_dir else {
        return model_path.to_string_lossy().into_owned();
    };
    if model_path.is_absolute()
        && let Ok(relative) = model_path.strip_prefix(project_dir)
    {
        return relative.to_string_lossy().into_owned();
    }
    model_path.to_string_lossy().into_owned()
}

/// A process-wide counter that gives every save its own temp file name.
///
/// F1.24. The name used to be `.rs_cam_save_{pid}.tmp`, so two saves running
/// at the same time in one process wrote ONE file: the first rename could
/// publish the other save's bytes and report success, and the second rename
/// then failed with `NotFound`. The counter gives every CALL a distinct
/// name, and the process id keeps two processes apart. A timestamp would add
/// nothing to that guarantee, and it would make a temp file left by a killed
/// process permanent, because no later save could ever overwrite it.
static SAVE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Build the temp file name for one save.
///
/// The name stays hidden, and [`ProjectSession::save`] keeps the file in the
/// DESTINATION directory. An atomic rename cannot cross a filesystem
/// boundary, which is why the temp file is not in the system temp directory.
fn save_temp_file_name() -> String {
    let pid = std::process::id();
    let seq = SAVE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(".rs_cam_save_{pid}_{seq}.tmp")
}

impl ProjectSession {
    /// Save the current session state to a TOML project file.
    ///
    /// The file is written atomically: contents go to a temporary file in the
    /// same directory, then renamed into place. The temp name is unique per
    /// CALL — see `save_temp_file_name` — so two saves from one process
    /// into one directory do not share it. A failed save removes its own
    /// temp file.
    #[instrument(skip(self))]
    pub fn save(&self, path: &Path) -> Result<(), SessionError> {
        // G-MODELRELINK (F4.3): the destination directory decides how model
        // paths are written — see `persist_model_path`. `save` is the only
        // caller that knows it, which is why the rule lives here and not in
        // `to_project_file`.
        let project = self.to_project_file(path.parent());
        let toml_string = toml::to_string_pretty(&project)
            .map_err(|e| SessionError::TomlSerialize(e.to_string()))?;

        // Atomic write: temp file in the same directory, then rename. The
        // name is unique per call, so two saves from one process into one
        // directory never share it (F1.24).
        let parent = path.parent().unwrap_or(Path::new("."));
        let temp_path = parent.join(save_temp_file_name());

        if let Err(e) = std::fs::write(&temp_path, &toml_string) {
            // A failed write can still leave a partial file behind.
            let _ = std::fs::remove_file(&temp_path);
            return Err(e.into());
        }
        if let Err(e) = std::fs::rename(&temp_path, path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(e.into());
        }

        Ok(())
    }

    /// Construct a `ProjectFile` from the current session state.
    ///
    /// This is the reverse of `build_session_from_project`.
    fn to_project_file(&self, project_dir: Option<&Path>) -> ProjectFile {
        // Stock
        let stock = ProjectStockConfig {
            x: self.stock.x,
            y: self.stock.y,
            z: self.stock.z,
            origin_x: self.stock.origin_x,
            origin_y: self.stock.origin_y,
            origin_z: self.stock.origin_z,
            padding: self.stock.padding,
            workholding_rigidity: self.stock.workholding_rigidity,
            auto_from_model: self.stock.auto_from_model,
            material: self.stock.material.clone(),
            alignment_pins: self.stock.alignment_pins.clone(),
            flip_axis: self.stock.flip_axis,
        };

        // Post
        let post = ProjectPostConfig {
            format: self.post.format.clone(),
            spindle_speed: self.post.spindle_speed,
            safe_z: self.post.safe_z,
            high_feedrate_mode: self.post.high_feedrate_mode,
            high_feedrate: self.post.high_feedrate,
            spindle_strategy: self.post.spindle_strategy,
        };

        // Job
        let job = ProjectJobSection {
            name: self.name.clone(),
            stock,
            post,
            machine: self.machine.clone(),
            machine_ref: self.machine_ref.clone(),
        };

        // Tools
        let tools: Vec<ProjectToolSection> = self
            .tools
            .iter()
            .map(|t| ProjectToolSection {
                id: Some(t.id.0),
                name: t.name.clone(),
                tool_type: tool_type_to_key(t.tool_type),
                diameter: t.diameter,
                cutting_length: t.cutting_length,
                helix_deg: t.helix_deg,
                corner_radius_mm: t.corner_radius_mm,
                corner_radius: t.corner_radius,
                included_angle: t.included_angle,
                taper_half_angle: t.taper_half_angle,
                shaft_diameter: t.shaft_diameter,
                holder_diameter: t.holder_diameter,
                shank_diameter: t.shank_diameter,
                shank_length: t.shank_length,
                stickout: t.stickout,
                flute_count: t.flute_count,
                tool_number: Some(t.tool_number as usize),
                tool_material: tool_material_to_key(t.tool_material),
                cut_direction: cut_direction_to_key(t.cut_direction),
                vendor: t.vendor.clone(),
                product_id: t.product_id.clone(),
            })
            .collect();

        // Models
        let models: Vec<ProjectModelSection> = self
            .models
            .iter()
            .map(|m| ProjectModelSection {
                id: Some(m.id),
                path: persist_model_path(project_dir, &m.path),
                name: m.name.clone(),
                kind: m.kind,
                units: m.units,
            })
            .collect();

        // Setups (with inline toolpaths)
        let setups: Vec<ProjectSetupSection> = self
            .setups
            .iter()
            .map(|s| {
                let toolpaths: Vec<ProjectToolpathSection> = s
                    .toolpath_indices
                    .iter()
                    .filter_map(|&tp_idx| self.toolpath_configs.get(tp_idx))
                    .map(|tc| ProjectToolpathSection {
                        id: Some(tc.id),
                        name: tc.name.clone(),
                        op_type: Some(tc.operation.op_type()),
                        operation: Some(tc.operation.clone()),
                        enabled: tc.enabled,
                        tool_id: Some(tc.tool_id),
                        model_id: Some(tc.model_id),
                        dressups: tc.dressups.clone(),
                        heights: tc.heights.clone(),
                        pre_gcode: tc.pre_gcode.clone(),
                        post_gcode: tc.post_gcode.clone(),
                        boundary: tc.boundary.clone(),
                        boundary_inherit: tc.boundary_inherit,
                        stock_source: tc.stock_source,
                        coolant: tc.coolant,
                        face_selection: tc
                            .face_selection
                            .as_ref()
                            .map(|ids| ids.iter().map(|fg| fg.0).collect()),
                        _legacy_feeds_auto: None,
                        debug_options: tc.debug_options,
                        feeds_provenance: tc.feeds_provenance.clone(),
                        rest_analysis: tc.rest_analysis.clone(),
                        planner_origin: tc.planner_origin.clone(),
                    })
                    .collect();

                let fixtures: Vec<ProjectFixtureSection> = s
                    .fixtures
                    .iter()
                    .map(|f| ProjectFixtureSection {
                        id: Some(f.id.0),
                        name: f.name.clone(),
                        kind: format!("{:?}", f.kind).to_ascii_lowercase(),
                        enabled: f.enabled,
                        origin_x: f.origin_x,
                        origin_y: f.origin_y,
                        origin_z: f.origin_z,
                        size_x: f.size_x,
                        size_y: f.size_y,
                        size_z: f.size_z,
                        clearance: f.clearance,
                    })
                    .collect();

                let keep_out_zones: Vec<ProjectKeepOutSection> = s
                    .keep_out_zones
                    .iter()
                    .map(|k| ProjectKeepOutSection {
                        id: Some(k.id.0),
                        name: k.name.clone(),
                        enabled: k.enabled,
                        origin_x: k.origin_x,
                        origin_y: k.origin_y,
                        size_x: k.size_x,
                        size_y: k.size_y,
                    })
                    .collect();

                // W9 / P-2. An untouched datum writes NO keys — the
                // three `skip_serializing_if = "String::is_empty"`
                // fields stay empty — so projects that never opened the
                // Setup panel serialise exactly as they did before.
                let (xy_datum, z_datum, datum_notes) = if s.datum.is_default() {
                    (String::new(), String::new(), String::new())
                } else {
                    (
                        s.datum.xy_method.to_key(),
                        s.datum.z_method.to_key(),
                        s.datum.notes.clone(),
                    )
                };

                ProjectSetupSection {
                    id: Some(s.id),
                    name: s.name.clone(),
                    face_up: s.face_up.to_key().to_owned(),
                    z_rotation: s.z_rotation.to_key().to_owned(),
                    pause_message: s.pause_message.clone(),
                    xy_datum,
                    z_datum,
                    datum_notes,
                    model_ids: s.model_ids.iter().map(|id| id.0).collect(),
                    fixtures,
                    keep_out_zones,
                    toolpaths,
                }
            })
            .collect();

        ProjectFile {
            format_version: 3,
            job,
            tools,
            models,
            setups,
            toolpaths: Vec::new(), // format_version=3 uses setups, not top-level toolpaths
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::stock_config::FixtureId;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::compute::transform::FaceUp;
    use crate::ids::ToolpathId;
    use crate::session::{Fixture, FixtureKind, KeepOutZone, ToolpathConfig};

    fn make_tc(tool_id: usize, model_id: usize) -> ToolpathConfig {
        use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
        ToolpathConfig {
            id: ToolpathId(0),
            name: "Test Op".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig::default()),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: crate::gcode::CoolantMode::Off,
            face_selection: None,
            debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            planner_origin: None,
        }
    }

    /// ONE directory for the whole test module, and one file per test.
    ///
    /// These tests run in parallel threads of one process, and they all save
    /// into this one directory. That is the case `save`'s per-call temp name
    /// exists for (F1.24), so the helper exercises it instead of avoiding
    /// it. Each test used to get a directory of its own, which was a
    /// workaround for the shared `.rs_cam_save_{pid}.tmp` name.
    fn temp_path(name: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let mut dir = std::env::temp_dir();
        dir.push(format!("rs_cam_test_save_{pid}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir.push(format!("{name}.toml"));
        dir
    }

    /// Remove one project file.
    ///
    /// The shared directory stays. One test that removed it while another
    /// test was between `create_dir_all` and `save` would break that test's
    /// write.
    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_session_round_trip() {
        let mut s = ProjectSession::new_empty();
        s.set_name("Test Project".to_owned());

        let path = temp_path("empty");
        s.save(&path).unwrap();
        let loaded = ProjectSession::load(&path).unwrap();

        assert_eq!(loaded.name(), "Test Project");
        assert_eq!(loaded.toolpath_count(), 0);
        // Empty sessions may have 0 or 1 default setups after load
        assert!(loaded.list_tools().is_empty());
        cleanup(&path);
    }

    #[test]
    fn tool_round_trip() {
        let mut s = ProjectSession::new_empty();
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
        tool.name = "2mm Ball".to_owned();
        tool.diameter = 2.0;
        tool.flute_count = 2;
        tool.stickout = 20.0;
        s.add_tool(tool);

        let path = temp_path("tool");
        s.save(&path).unwrap();
        let loaded = ProjectSession::load(&path).unwrap();

        assert_eq!(loaded.tools().len(), 1);
        let loaded_tool = &loaded.tools()[0];
        assert_eq!(loaded_tool.name, "2mm Ball");
        assert!((loaded_tool.diameter - 2.0).abs() < 1e-9);
        assert_eq!(loaded_tool.flute_count, 2);
        assert!((loaded_tool.stickout - 20.0).abs() < 1e-9);
        assert!(matches!(loaded_tool.tool_type, ToolType::BallNose));
        cleanup(&path);
    }

    #[test]
    fn setup_with_fixture_and_keep_out_round_trip() {
        let mut s = ProjectSession::new_empty();
        let fixture = Fixture {
            id: FixtureId(0),
            name: "Clamp 1".to_owned(),
            kind: FixtureKind::Clamp,
            enabled: true,
            origin_x: 10.0,
            origin_y: 20.0,
            origin_z: 0.0,
            size_x: 30.0,
            size_y: 15.0,
            size_z: 20.0,
            clearance: 3.0,
        };
        let _ = s.add_fixture(0, fixture).unwrap();

        let zone = KeepOutZone {
            id: crate::compute::stock_config::KeepOutId(0),
            name: "Danger Zone".to_owned(),
            enabled: true,
            origin_x: 5.0,
            origin_y: 5.0,
            size_x: 15.0,
            size_y: 25.0,
        };
        let _ = s.add_keep_out(0, zone).unwrap();

        let path = temp_path("setup_fixture");
        s.save(&path).unwrap();
        let loaded = ProjectSession::load(&path).unwrap();

        let setup = &loaded.list_setups()[0];
        assert_eq!(setup.fixtures.len(), 1);
        assert_eq!(setup.fixtures[0].name, "Clamp 1");
        assert!((setup.fixtures[0].origin_x - 10.0).abs() < 1e-9);
        assert!((setup.fixtures[0].clearance - 3.0).abs() < 1e-9);

        assert_eq!(setup.keep_out_zones.len(), 1);
        assert_eq!(setup.keep_out_zones[0].name, "Danger Zone");
        assert!((setup.keep_out_zones[0].size_y - 25.0).abs() < 1e-9);
        cleanup(&path);
    }

    #[test]
    fn toolpath_round_trip() {
        let mut s = ProjectSession::new_empty();
        s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let tool_id = s.tools()[0].id.0;

        let mut tc = make_tc(tool_id, 0);
        tc.name = "My Pocket".to_owned();
        tc.enabled = false;
        s.add_toolpath(0, tc).unwrap();

        let path = temp_path("toolpath");
        s.save(&path).unwrap();
        let loaded = ProjectSession::load(&path).unwrap();

        assert_eq!(loaded.toolpath_count(), 1);
        let loaded_tc = &loaded.toolpath_configs()[0];
        assert_eq!(loaded_tc.name, "My Pocket");
        assert!(!loaded_tc.enabled);
        assert!(matches!(loaded_tc.operation, OperationConfig::Pocket(_)));
        cleanup(&path);
    }

    #[test]
    fn stock_config_round_trip() {
        let mut s = ProjectSession::new_empty();
        let mut stock = s.stock_config().clone();
        stock.x = 150.0;
        stock.y = 200.0;
        stock.z = 25.0;
        stock.origin_x = -10.0;
        stock.padding = 5.0;
        let _ = s.set_stock_config(stock);

        let path = temp_path("stock");
        s.save(&path).unwrap();
        let loaded = ProjectSession::load(&path).unwrap();

        let loaded_stock = loaded.stock_config();
        assert!((loaded_stock.x - 150.0).abs() < 1e-9);
        assert!((loaded_stock.y - 200.0).abs() < 1e-9);
        assert!((loaded_stock.z - 25.0).abs() < 1e-9);
        assert!((loaded_stock.origin_x - (-10.0)).abs() < 1e-9);
        assert!((loaded_stock.padding - 5.0).abs() < 1e-9);
        cleanup(&path);
    }

    #[test]
    fn multi_setup_round_trip() {
        let mut s = ProjectSession::new_empty();
        s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let tool_id = s.tools()[0].id.0;

        s.add_setup("Bottom Setup".to_owned(), FaceUp::Bottom);
        assert_eq!(s.list_setups().len(), 2);

        // One toolpath in each setup
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.add_toolpath(1, make_tc(tool_id, 0)).unwrap();

        let path = temp_path("multi_setup");
        s.save(&path).unwrap();
        let loaded = ProjectSession::load(&path).unwrap();

        assert_eq!(loaded.list_setups().len(), 2);
        assert_eq!(loaded.toolpath_count(), 2);
        assert_eq!(loaded.list_setups()[0].toolpath_indices.len(), 1);
        assert_eq!(loaded.list_setups()[1].toolpath_indices.len(), 1);
        assert!(matches!(loaded.list_setups()[1].face_up, FaceUp::Bottom));
        cleanup(&path);
    }

    /// `pause_message = None` (the default) MUST not appear in the serialized
    /// TOML, so old project files round-trip byte-for-byte. A `Some(...)`
    /// override appears under `[setups]` and loads back unchanged.
    #[test]
    fn setup_pause_message_round_trip() {
        let mut s = ProjectSession::new_empty();
        s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let tool_id = s.tools()[0].id.0;
        s.add_setup("Bottom Setup".to_owned(), FaceUp::Bottom);
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.add_toolpath(1, make_tc(tool_id, 0)).unwrap();

        // Default: no pause_message — must not appear in serialized TOML.
        let path_none = temp_path("pause_none");
        s.save(&path_none).unwrap();
        let serialized = std::fs::read_to_string(&path_none).unwrap();
        assert!(
            !serialized.contains("pause_message"),
            "TOML for default pause_message=None must not include the key; got:\n{serialized}"
        );
        let loaded = ProjectSession::load(&path_none).unwrap();
        assert!(
            loaded
                .list_setups()
                .iter()
                .all(|s| s.pause_message.is_none())
        );
        cleanup(&path_none);

        // Override: setup[1].pause_message = Some("Run Z Probe macro then Resume")
        s.setups_mut()[1].pause_message = Some("Run Z Probe macro then Resume".to_owned());
        let path_some = temp_path("pause_some");
        s.save(&path_some).unwrap();
        let serialized = std::fs::read_to_string(&path_some).unwrap();
        assert!(
            serialized.contains("pause_message"),
            "TOML must include pause_message when override is set; got:\n{serialized}"
        );
        let loaded = ProjectSession::load(&path_some).unwrap();
        assert_eq!(loaded.list_setups()[0].pause_message, None);
        assert_eq!(
            loaded.list_setups()[1].pause_message.as_deref(),
            Some("Run Z Probe macro then Resume"),
        );
        cleanup(&path_some);
    }

    /// A toolpath without a spindle_rpm override saves WITHOUT the
    /// `spindle_rpm` key (skip_serializing_if = "Option::is_none") and
    /// loads back as `None`. Old project files (which never had the
    /// field) therefore round-trip unchanged via #[serde(default)].
    #[test]
    fn toolpath_without_spindle_rpm_round_trip() {
        let mut s = ProjectSession::new_empty();
        s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let tool_id = s.tools()[0].id.0;

        let tc = make_tc(tool_id, 0);
        // sanity: default PocketConfig has no override
        assert!(matches!(
            &tc.operation,
            OperationConfig::Pocket(p) if p.spindle_rpm.is_none()
        ));
        s.add_toolpath(0, tc).unwrap();

        let path = temp_path("spindle_rpm_unset");
        s.save(&path).unwrap();

        // The serialized TOML should not mention spindle_rpm at all.
        let serialized = std::fs::read_to_string(&path).unwrap();
        assert!(
            !serialized.contains("spindle_rpm"),
            "TOML for a default operation must not include spindle_rpm; got:\n{serialized}"
        );

        let loaded = ProjectSession::load(&path).unwrap();
        assert_eq!(loaded.toolpath_count(), 1);
        let op = &loaded.toolpath_configs()[0].operation;
        assert_eq!(op.spindle_rpm(), None);
        cleanup(&path);
    }

    /// A toolpath with `spindle_rpm = Some(12000)` serializes the field
    /// and loads back to the same value.
    #[test]
    fn toolpath_with_spindle_rpm_round_trip() {
        let mut s = ProjectSession::new_empty();
        s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let tool_id = s.tools()[0].id.0;

        let mut tc = make_tc(tool_id, 0);
        if let OperationConfig::Pocket(p) = &mut tc.operation {
            p.spindle_rpm = Some(12_000);
        }
        s.add_toolpath(0, tc).unwrap();

        let path = temp_path("spindle_rpm_set");
        s.save(&path).unwrap();

        let serialized = std::fs::read_to_string(&path).unwrap();
        assert!(
            serialized.contains("spindle_rpm"),
            "TOML must include spindle_rpm when override is set; got:\n{serialized}"
        );
        assert!(
            serialized.contains("12000"),
            "TOML must contain the override value; got:\n{serialized}"
        );

        let loaded = ProjectSession::load(&path).unwrap();
        let op = &loaded.toolpath_configs()[0].operation;
        assert_eq!(op.spindle_rpm(), Some(12_000));
        cleanup(&path);
    }
}
