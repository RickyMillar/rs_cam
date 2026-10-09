//! S4 (`planning/stock_additions_2026-10-09/PLAN.md`): the stock-change
//! rows that every surface prints.
//!
//! The GUI setup panel, the MCP `list_setups` and `inspect_stock` replies
//! and the CLI `project` summary read [`ProjectSession::stock_change_rows`].
//! The volume of a row comes from the session's simulation
//! (`SimulationResult::stock_change_volumes`), and its text comes from
//! [`StockChangeVolume::volume_label`]. So one project gives the same
//! numbers on every surface (GUI/MCP/CLI number parity).

use crate::compute::stock_change::{StockChange, StockChangeOp, StockGeometry};
use crate::compute::stock_change_apply::StockChangeVolume;
use crate::export::material_colour::{MaterialPalette, colour_from_rgb8};
use crate::stock::material_slot::MaterialSlot;

use super::ProjectSession;

/// Why a row has no volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockChangeVolumeAbsence {
    /// The change is disabled, so the simulation does not apply it.
    Disabled,
    /// The session holds no simulation that applied the change.
    NotSimulated,
}

impl StockChangeVolumeAbsence {
    /// The text a surface prints in place of the volume.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NotSimulated => "not simulated",
        }
    }
}

/// One stock change of one setup, with the volume the last simulation
/// measured for it.
#[derive(Debug, Clone)]
pub struct StockChangeRow {
    /// The index of the setup in `ProjectSession::list_setups`.
    pub setup_index: usize,
    /// The `SetupData::id` of the setup.
    pub setup_id: usize,
    /// The setup name.
    pub setup_name: String,
    /// The position of the change in the setup's list (application order).
    pub position: usize,
    /// The change record.
    pub change: StockChange,
    /// The volume the simulation measured, or why there is none.
    pub volume: Result<StockChangeVolume, StockChangeVolumeAbsence>,
}

impl StockChangeRow {
    /// The volume text: [`StockChangeVolume::volume_label`], or the
    /// absence label.
    #[must_use]
    pub fn volume_label(&self) -> String {
        match &self.volume {
            Ok(volume) => volume.volume_label(),
            Err(absence) => absence.label().to_owned(),
        }
    }

    /// The material text. A `Remove` ignores its material, so it prints
    /// none.
    #[must_use]
    pub fn material_label(&self) -> String {
        match self.change.op {
            crate::compute::stock_change::StockChangeOp::Add => self.change.material.label(),
            crate::compute::stock_change::StockChangeOp::Remove => "-".to_owned(),
        }
    }

    /// S6: the "cut as" text of an `Add`: `cut as stock` or
    /// `own material`. A `Remove` ignores `cut_as`, so it gives `None`.
    #[must_use]
    pub fn cut_as_label(&self) -> Option<&'static str> {
        match self.change.op {
            crate::compute::stock_change::StockChangeOp::Add => Some(self.change.cut_as.label()),
            crate::compute::stock_change::StockChangeOp::Remove => None,
        }
    }

    /// The model ids the geometry reads, as `3, 4`.
    #[must_use]
    pub fn model_ids_label(&self) -> String {
        self.change
            .geometry
            .model_ids()
            .iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The Z values of the geometry, in the setup frame (mm).
    #[must_use]
    pub fn z_label(&self) -> String {
        match &self.change.geometry {
            StockGeometry::Model { .. } => "-".to_owned(),
            StockGeometry::OutlineFill { level_z, .. } => format!("level Z {level_z:.3}"),
            StockGeometry::OutlineExtrude {
                z_bottom, z_top, ..
            } => {
                format!("Z {z_bottom:.3} to {z_top:.3}")
            }
        }
    }

    /// The one-line summary that the CLI prints. An `Add` names its
    /// "cut as" after the material: `material Resin (cut as stock)`.
    #[must_use]
    pub fn line(&self) -> String {
        let cut_as = self
            .cut_as_label()
            .map_or_else(String::new, |label| format!(" ({label})"));
        format!(
            "setup {} '{}' #{} '{}': {} {} (models {}; {}), material {}{}: {}",
            self.setup_index,
            self.setup_name,
            self.change.id.0,
            self.change.name,
            self.change.op.label(),
            self.change.geometry.kind_label(),
            self.model_ids_label(),
            self.z_label(),
            self.material_label(),
            cut_as,
            self.volume_label(),
        )
    }
}

/// One material of the simulated stock, for the stock legend (S5).
#[derive(Debug, Clone, PartialEq)]
pub struct StockMaterialSwatch {
    /// The material slot (0 = the stock material).
    pub slot: MaterialSlot,
    /// The material name (`Material::label`).
    pub label: String,
    /// The colour at the uncut surface, as the stock views draw it.
    pub colour: [f32; 3],
}

impl ProjectSession {
    /// The colour of each material slot of the simulated stock (S5).
    ///
    /// Slot `k` comes from the default palette, or from the
    /// `display_colour` of the first applied `Add` change (in application
    /// order) that wrote slot `k` and has one. The slot of a change comes
    /// from the simulation (`StockChangeVolume::material_slot`). With no
    /// simulation, the palette is the default.
    #[must_use]
    pub fn stock_material_palette(&self) -> MaterialPalette {
        let mut palette = MaterialPalette::default();
        let mut coloured = [false; crate::stock::material_slot::MATERIAL_SLOT_CAPACITY];
        for (slot, change) in self.applied_additions() {
            let Some(rgb) = change.display_colour else {
                continue;
            };
            if let Some(done) = coloured.get_mut(slot.index())
                && !*done
            {
                *done = true;
                palette = palette.with_colour(slot, colour_from_rgb8(rgb));
            }
        }
        palette
    }

    /// The materials of the simulated stock, slot 0 first, with the colour
    /// that the stock views draw them in (S5). Empty when the simulation
    /// added no material: a one-material stock needs no legend.
    #[must_use]
    pub fn stock_material_legend(&self) -> Vec<StockMaterialSwatch> {
        let palette = self.stock_material_palette();
        let mut swatches: Vec<StockMaterialSwatch> = Vec::new();
        for (slot, change) in self.applied_additions() {
            if swatches.iter().any(|s| s.slot == slot) {
                continue;
            }
            swatches.push(StockMaterialSwatch {
                slot,
                label: change.material.label(),
                colour: palette.colour(slot),
            });
        }
        if swatches.is_empty() {
            return swatches;
        }
        swatches.sort_by_key(|s| s.slot);
        swatches.insert(
            0,
            StockMaterialSwatch {
                slot: MaterialSlot::STOCK,
                label: self.stock_config().material.label(),
                colour: palette.colour(MaterialSlot::STOCK),
            },
        );
        swatches
    }

    /// Each `Add` change that the simulation applied and that added
    /// material, with its slot, in application order.
    fn applied_additions(&self) -> Vec<(MaterialSlot, &StockChange)> {
        let Some(sim) = self.simulation_result() else {
            return Vec::new();
        };
        let setups = self.list_setups();
        sim.stock_change_volumes
            .iter()
            .filter(|v| v.op == StockChangeOp::Add && v.added_mm3 > 0.0)
            .filter_map(|v| {
                let slot = v.material_slot.filter(|s| !s.is_stock())?;
                let change = setups
                    .iter()
                    .find(|s| s.id == v.setup_id)?
                    .stock_changes
                    .iter()
                    .find(|c| c.id == v.change_id)?;
                Some((slot, change))
            })
            .collect()
    }

    /// Every stock change of every setup, in setup order and, within a
    /// setup, in application order, with the volume the session's
    /// simulation measured for it.
    ///
    /// A stock-change edit drops the simulation, so a volume that is
    /// present belongs to the change as it is now.
    #[must_use]
    pub fn stock_change_rows(&self) -> Vec<StockChangeRow> {
        let volumes: &[StockChangeVolume] = self
            .simulation_result()
            .map_or(&[], |sim| sim.stock_change_volumes.as_slice());
        let mut rows = Vec::new();
        for (setup_index, setup) in self.list_setups().iter().enumerate() {
            for (position, change) in setup.stock_changes.iter().enumerate() {
                let volume = if change.enabled {
                    volumes
                        .iter()
                        .find(|v| v.setup_id == setup.id && v.change_id == change.id)
                        .cloned()
                        .ok_or(StockChangeVolumeAbsence::NotSimulated)
                } else {
                    Err(StockChangeVolumeAbsence::Disabled)
                };
                rows.push(StockChangeRow {
                    setup_index,
                    setup_id: setup.id,
                    setup_name: setup.name.clone(),
                    position,
                    change: change.clone(),
                    volume,
                });
            }
        }
        rows
    }
}
