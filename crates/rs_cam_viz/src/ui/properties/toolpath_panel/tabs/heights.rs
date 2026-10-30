//! The Heights tab of the toolpath inspector.

use super::op_report;
use crate::ui::AppEvent;
use crate::ui::properties::operations;
use crate::ui::properties::operations::{draw_height_diagram, draw_heights_params};

/// The Heights tab: the Z planes.
///
/// P3.1: the diagram and the rows read THE heights of the session
/// (`ProjectSession::toolpath_heights`). When they do not resolve, the tab
/// names the error and draws no row: a row needs the stock and the model Z.
pub(in crate::ui::properties::toolpath_panel) fn draw_heights_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &crate::ui::properties::ToolpathPanelInputs,
    heights_refusal: &mut crate::state::panels::HeightsRefusal,
    events: &mut Vec<AppEvent>,
) {
    heights_refusal.scope(entry.id);
    let answer = match inputs.toolpath_heights.as_ref() {
        Some(Ok(answer)) => Some(answer),
        Some(Err(error)) => {
            ui.add(crate::ui::components::OneLine::new(format!(
                "Heights: {error}"
            )));
            None
        }
        None => None,
    };
    // F1.19 / G-BOTTOMPIN: the Bottom row is annotated per operation,
    // so the panel needs the operation. Read before the mutable
    // borrow of `entry.heights`; `OperationType` is `Copy`.
    // F-6 / R33: the diagram takes the same operation, so its Bottom
    // line agrees with the sentence the panel prints beside the row.
    let op_type = entry.operation.op_type();
    // P3.7, D6: the floor of an `OpFloor` op. The diagram drags it in its
    // own reference (L-10), and the Bottom height row under the Top height
    // row edits it. Both write ONE draft floor; it goes back through the
    // config after both, as every op row does.
    let floor_inputs = answer.and_then(|answer| {
        entry.operation.floor().map(|_| {
            crate::ui::properties::FloorInputs::new(answer, &entry.heights, op_report(inputs))
        })
    });
    // G-DEPTHSTOCK: the caution of a floor below the stock bottom goes
    // with the Bottom height row. Read before the mutable borrows.
    let depth_caution = answer.and_then(|answer| {
        operations::depth_beyond_stock(&entry.operation, &entry.heights, answer)
    });
    let mut floor = entry.operation.floor();
    // IC1: the diagram summarises the five planes, so it comes first. The
    // rows below it set them.
    if let Some(answer) = answer {
        let diagram_floor = floor
            .as_mut()
            .zip(floor_inputs.as_ref())
            .map(|(floor, inputs)| crate::ui::properties::DiagramFloor { floor, inputs });
        draw_height_diagram(
            ui,
            &mut entry.heights,
            answer,
            op_type,
            diagram_floor,
            heights_refusal,
        );
        ui.add_space(6.0);
    }
    draw_heights_params(
        ui,
        &mut entry.heights,
        answer,
        op_type,
        floor.as_mut().map(|floor| operations::BottomHeight {
            floor,
            inputs: floor_inputs.as_ref(),
            depth_caution: depth_caution.as_ref(),
        }),
        heights_refusal,
    );
    if let Some(floor) = floor
        && entry.operation.floor().as_ref() != Some(&floor)
    {
        let _ = entry.operation.set_floor(floor);
    }
    // H2: the dust shoe sits with the safety heights. It moves no
    // toolpath; it decides what the checks read above the tool.
    if let Some(shoe) = inputs.shoe.as_ref() {
        ui.add_space(6.0);
        crate::ui::properties::shoe_row::draw_shoe_row(ui, entry.id, shoe, events);
    }
}
