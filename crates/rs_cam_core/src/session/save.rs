//! Save a [`ProjectSession`] back to a TOML project file.

use std::path::Path;

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

impl ProjectSession {
    /// Save the current session state to a TOML project file.
    ///
    /// The file is written atomically: contents go to a temporary file in the
    /// same directory, then renamed into place.
    pub fn save(&self, path: &Path) -> Result<(), SessionError> {
        let project = self.to_project_file();
        let toml_string = toml::to_string_pretty(&project)
            .map_err(|e| SessionError::TomlSerialize(e.to_string()))?;

        // Atomic write: temp file in the same directory, then rename.
        let parent = path.parent().unwrap_or(Path::new("."));
        let temp_path = parent.join(format!(".rs_cam_save_{}.tmp", std::process::id()));

        std::fs::write(&temp_path, &toml_string)?;
        std::fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Construct a `ProjectFile` from the current session state.
    ///
    /// This is the reverse of `build_session_from_project`.
    fn to_project_file(&self) -> ProjectFile {
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
        };

        // Job
        let job = ProjectJobSection {
            name: self.name.clone(),
            stock,
            post,
            machine: self.machine.clone(),
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
                path: m.path.to_string_lossy().into_owned(),
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
                        feeds_auto: tc.feeds_auto.clone(),
                        debug_options: tc.debug_options,
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

                ProjectSetupSection {
                    id: Some(s.id),
                    name: s.name.clone(),
                    face_up: s.face_up.to_key().to_owned(),
                    z_rotation: s.z_rotation.to_key().to_owned(),
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
