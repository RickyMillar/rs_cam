use super::tool_config::{ToolConfig, ToolType};
use crate::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, ToolDefinition,
    VBitEndmill,
};

/// Build a `ToolDefinition` (cutter geometry + holder dimensions) from a `ToolConfig`.
pub fn build_cutter(tool: &ToolConfig) -> ToolDefinition {
    let cutter: Box<dyn MillingCutter> = match tool.tool_type {
        ToolType::EndMill => {
            let mut cutter = FlatEndmill::new(tool.diameter, tool.cutting_length);
            cutter.helix_deg = tool.helix_deg;
            cutter.corner_radius_mm = tool.corner_radius_mm;
            Box::new(cutter)
        }
        ToolType::BallNose => {
            let mut cutter = BallEndmill::new(tool.diameter, tool.cutting_length);
            cutter.helix_deg = tool.helix_deg;
            Box::new(cutter)
        }
        ToolType::BullNose => {
            let mut cutter =
                BullNoseEndmill::new(tool.diameter, tool.corner_radius, tool.cutting_length);
            cutter.helix_deg = tool.helix_deg;
            Box::new(cutter)
        }
        ToolType::VBit => {
            let mut cutter =
                VBitEndmill::new(tool.diameter, tool.included_angle, tool.cutting_length);
            cutter.helix_deg = tool.helix_deg;
            Box::new(cutter)
        }
        ToolType::TaperedBallNose => {
            let mut cutter = TaperedBallEndmill::new(
                tool.diameter,
                tool.taper_half_angle,
                tool.shaft_diameter,
                tool.cutting_length,
            );
            cutter.helix_deg = tool.helix_deg;
            Box::new(cutter)
        }
    };
    ToolDefinition::new(
        cutter,
        tool.shank_diameter,
        tool.shank_length,
        tool.holder_diameter,
        tool.stickout,
        tool.flute_count,
        tool.tool_material,
    )
}
