//! S4 (`planning/stock_additions_2026-10-09/PLAN.md`): the stock-change
//! rows that every surface prints.
//!
//! The GUI setup panel, the MCP `list_setups` and `inspect_stock` replies
//! and the CLI `project` summary read [`ProjectSession::stock_change_rows`].
//! The volume of a row comes from the session's simulation
//! (`SimulationResult::stock_change_volumes`), and its text comes from
//! [`StockChangeVolume::volume_label`]. So one project gives the same
//! numbers on every surface (GUI/MCP/CLI number parity).

use crate::compute::stock_change::{StockChange, StockGeometry};
use crate::compute::stock_change_apply::StockChangeVolume;

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

    /// The one-line summary that the CLI prints.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "setup {} '{}' #{} '{}': {} {} (models {}; {}), material {}: {}",
            self.setup_index,
            self.setup_name,
            self.change.id.0,
            self.change.name,
            self.change.op.label(),
            self.change.geometry.kind_label(),
            self.model_ids_label(),
            self.z_label(),
            self.material_label(),
            self.volume_label(),
        )
    }
}

impl ProjectSession {
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
