use super::tool_config::{ToolConfig, ToolType};
use crate::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, ToolDefinition,
    VBitEndmill,
};

/// Build a `ToolDefinition` (cutter geometry + holder dimensions) from a `ToolConfig`.
pub fn build_cutter(tool: &ToolConfig) -> ToolDefinition {
    let cutter: Box<dyn MillingCutter> = match tool.tool_type {
        ToolType::EndMill => Box::new(FlatEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BallNose => Box::new(BallEndmill::new(tool.diameter, tool.cutting_length)),
        ToolType::BullNose => Box::new(BullNoseEndmill::new(
            tool.diameter,
            tool.corner_radius,
            tool.cutting_length,
        )),
        ToolType::VBit => Box::new(VBitEndmill::new(
            tool.diameter,
            tool.included_angle,
            tool.cutting_length,
        )),
        ToolType::TaperedBallNose => Box::new(TaperedBallEndmill::new(
            tool.diameter,
            tool.taper_half_angle,
            tool.shaft_diameter,
            tool.cutting_length,
        )),
    };
    ToolDefinition::new(
        cutter,
        tool.shank_diameter,
        tool.shank_length,
        tool.holder_diameter,
        tool.stickout,
        tool.flute_count,
    )
}
