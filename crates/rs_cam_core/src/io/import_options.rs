//! The import options of a model and the report of a drawing import
//! (geometry selection P4.8).
//!
//! The options go into the project file with the model, so that a reload,
//! a rescale and a relink read the file with the same options and give the
//! same items. The report does not go into the file: each load computes it.

use serde::{Deserialize, Serialize};

use crate::selection::ItemSet;
use crate::selection::join::{CLOSE_TOL_MM, JoinCounts, LayerJoin};

/// The options of one model import. The project file writes them under
/// `[models.import]`, and only when they are not the default, so a project
/// with default options saves as before P4.8.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImportOptions {
    /// Two open piece ends closer than this join (DXF and SVG, ruling O-5),
    /// in mm of the file as the door reads it: after the DXF `$INSUNITS`
    /// scale, and before the declared [`ModelUnits`] scale. At the default
    /// declared units (mm) this is mm of the model.
    ///
    /// [`ModelUnits`]: crate::compute::stock_config::ModelUnits
    #[serde(default = "default_join_tolerance_mm")]
    pub join_tolerance_mm: f64,
}

fn default_join_tolerance_mm() -> f64 {
    CLOSE_TOL_MM
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            join_tolerance_mm: CLOSE_TOL_MM,
        }
    }
}

impl ImportOptions {
    /// True when every option has its default value (serde skips it).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Refuse an option that no import can use: a join tolerance that is
    /// not finite and positive.
    pub fn validate(&self) -> Result<(), String> {
        if self.join_tolerance_mm.is_finite() && self.join_tolerance_mm > 0.0 {
            Ok(())
        } else {
            Err(format!(
                "the join tolerance must be a positive number of mm, not {}",
                self.join_tolerance_mm
            ))
        }
    }
}

/// The report of one drawing import: what the join of open pieces did,
/// per layer. A mesh or a STEP import has no report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    /// The join tolerance of this import, mm (as [`ImportOptions`]).
    pub join_tolerance_mm: f64,
    /// One row for each layer that has an open piece, in the order of the
    /// first shape of the layer.
    pub layers: Vec<LayerJoin>,
}

impl ImportReport {
    /// The counts of every layer, added.
    pub fn total(&self) -> JoinCounts {
        self.layers
            .iter()
            .fold(JoinCounts::default(), |sum, row| sum.plus(row.counts))
    }

    /// The report as one line, for the model panel and the import dialog:
    /// "9 open pieces → 6 shapes · 1 joined · 1 closed · 5 open left".
    pub fn summary_line(&self) -> String {
        let total = self.total();
        if total.pieces == 0 {
            return "No open pieces".to_owned();
        }
        format!(
            "{} open pieces \u{2192} {} shapes \u{b7} {} joined \u{b7} {} closed \u{b7} {} open left",
            total.pieces, total.shapes, total.joined, total.closed_by_join, total.open_left
        )
    }
}

/// The result of a drawing door (DXF, SVG): the items and the join report.
#[derive(Debug, Clone)]
pub struct DrawingImport {
    /// The items in source order: a closed entity or subpath at its own
    /// index, a chain at the lowest index of its pieces. Each item carries
    /// its layer.
    pub items: ItemSet,
    /// What the join did.
    pub report: ImportReport,
}
