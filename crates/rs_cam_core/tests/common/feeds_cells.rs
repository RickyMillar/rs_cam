//! The feeds cell walk of the series sentries (operator ruling 2026-10-31).
//!
//! The walk follows the FM1 instrument (`feeds_matrix_instrument_fm1.rs`):
//! the same tool builder, the same add-time operation and the same Suggest
//! door (`suggest_for_operation` with the op's dressups). It adds 3 flutes
//! and the 6.35 mm and 12.7 mm end mills, because the series question is
//! asked of the common flat end mill sizes.

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{BitCutDirection, ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{StockContext, SuggestContext, SuggestedParams};
use rs_cam_core::feeds::{EMBEDDED_LUT, SpindleStrategy};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

/// One walked cell and the Suggest answer for it.
pub struct FeedsCell {
    pub kind: ToolType,
    pub diameter_mm: f64,
    pub flutes: u32,
    pub op: OperationType,
    pub material_label: &'static str,
    pub material: Material,
    pub tool: ToolConfig,
    /// The add-time operation that Suggest read.
    pub operation: OperationConfig,
    /// `None` when Suggest refused the cell.
    pub suggested: Option<SuggestedParams>,
}

impl FeedsCell {
    /// The cell name in a failure line.
    pub fn label(&self) -> String {
        format!(
            "{:?} {:.3} mm {}F {:?} {}",
            self.kind, self.diameter_mm, self.flutes, self.op, self.material_label
        )
    }
}

/// The five wood materials (the four of FM1 and soft plywood).
pub fn wood_materials() -> [(&'static str, Material); 5] {
    [
        (
            "softwood",
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
        ),
        (
            "hardwood",
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
        ),
        (
            "mdf",
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
        (
            "plywood_hardwood",
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
        ),
        (
            "plywood_softwood",
            Material::Plywood {
                grade: PlywoodGrade::Softwood,
            },
        ),
    ]
}

/// The diameters per tool type.
pub fn diameters(kind: ToolType) -> &'static [f64] {
    match kind {
        ToolType::EndMill => &[3.175, 6.0, 6.35, 12.7],
        ToolType::BallNose | ToolType::BullNose | ToolType::TaperedBallNose => &[3.175, 6.0],
        ToolType::VBit => &[6.35, 12.7],
    }
}

/// The FM1 tool builder.
pub fn tool_of(kind: ToolType, diameter: f64, flutes: u32) -> ToolConfig {
    let cutting_length = (diameter * 3.0).max(12.0);
    let mut t = ToolConfig::builder(ToolId::from_raw(1), kind)
        .diameter(diameter)
        .flutes(flutes)
        .flute_mm(cutting_length)
        .shank_mm(diameter.max(3.0))
        .shaft_mm(diameter.max(3.0))
        .stickout_mm(cutting_length + 8.0);
    if matches!(kind, ToolType::BullNose) {
        t = t
            .corner_radius(diameter * 0.15)
            .corner_radius_mm(diameter * 0.15);
    }
    if matches!(kind, ToolType::TaperedBallNose) {
        let shank = (diameter + 3.0).max(6.0);
        t = t.taper_half_angle_deg(7.0).shank_mm(shank).shaft_mm(shank);
    }
    if matches!(kind, ToolType::VBit) {
        t = t.included_angle_deg(60.0);
    }
    t.build()
}

/// The FM1 stock.
pub fn stock_ctx() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: Some(18.0),
        stock_padding: 2.0,
    }
}

/// The FM1 add-time operation.
pub fn add_time_op(op: OperationType, stock: &StockContext) -> OperationConfig {
    use rs_cam_core::compute::floor::Floor;
    let mut operation = rs_cam_core::feeds::suggest::default_operation(op, stock);
    let z = stock.stock_z.expect("the stock has a thickness");
    match &mut operation {
        OperationConfig::Pocket(c) => c.floor = Floor::top((z * 0.5).min(5.0)),
        OperationConfig::Adaptive(c) => c.floor = Floor::top(z * 0.5),
        OperationConfig::Face(_)
        | OperationConfig::Profile(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Drill(_)
        | OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::RampFinish(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_)
        | OperationConfig::ProjectCurve(_)
        | OperationConfig::AlignmentPinDrill(_) => {}
    }
    rs_cam_core::compute::fixture_values::fill_unset(&mut operation);
    operation.set_spindle_rpm(None);
    operation
}

/// Suggest for one cell through the product door.
pub fn suggest(
    operation: &OperationConfig,
    op: OperationType,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
) -> Option<SuggestedParams> {
    let dressups = rs_cam_core::compute::config::DressupConfig::for_op(op);
    rs_cam_core::feeds::suggest::suggest_for_operation(
        rs_cam_core::feeds::suggest::SuggestForOperationInput {
            operation,
            tool,
            machine,
            material,
            lut: &EMBEDDED_LUT,
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext {
                dressups: Some(&dressups),
                ..SuggestContext::default()
            },
        },
    )
    .ok()
}

/// Walk every cell: tool type x diameter x 2 and 3 flutes x operation x
/// wood material, with generic (upcut) tools.
pub fn walk() -> Vec<FeedsCell> {
    walk_with(BitCutDirection::UpCut)
}

/// [`walk`] with every tool set to `cut_direction`, which declares the
/// tool's cutter series (`feeds::series::ToolSeries::for_cut_direction`).
pub fn walk_with(cut_direction: BitCutDirection) -> Vec<FeedsCell> {
    let machine = MachineProfile::default();
    let stock = stock_ctx();
    let mut cells = Vec::new();
    for &kind in ToolType::ALL {
        for &diameter_mm in diameters(kind) {
            for flutes in [2, 3] {
                let mut tool = tool_of(kind, diameter_mm, flutes);
                tool.cut_direction = cut_direction;
                for &op in OperationType::ALL {
                    for (material_label, material) in wood_materials() {
                        let operation = add_time_op(op, &stock);
                        let suggested = suggest(&operation, op, &tool, &machine, &material);
                        cells.push(FeedsCell {
                            kind,
                            diameter_mm,
                            flutes,
                            op,
                            material_label,
                            material,
                            tool: tool.clone(),
                            operation,
                            suggested,
                        });
                    }
                }
            }
        }
    }
    cells
}
