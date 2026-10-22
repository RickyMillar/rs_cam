//! F31: the auto stock fits every model of the project.
//!
//! Operator ruling 2026-10-23: "stock should fit all models". This file
//! holds THE rule for an `auto_from_model` stock. Each door that changes
//! the model set calls [`fit_auto_stock`]: `add_model_impl`,
//! `remove_model`, `adopt_model_geometry` (re-import, rescale, import
//! options), `edit_model_drawing` and the project load. The stock panel
//! reads [`auto_stock_bbox`] for its refit.
//!
//! The functions take a slice of models and one stock, so a later list of
//! stocks can call them once per stock with that stock's models.

use crate::compute::stock_config::StockConfig;
use crate::geo::BoundingBox3;

use super::LoadedModel;

/// The box an auto stock fits: the union of the boxes of `models`.
///
/// The set is every model with geometry ([`LoadedModel::bbox`]). It is
/// not the pin part set (`ProjectSession::pin_part_models`): that set
/// reads the stock box, so a stock fitted to it would depend on itself.
/// A drawing gives a flat box at Z 0, so it adds XY extent and pulls the
/// Z range to include Z 0. `None` when no model has geometry.
#[must_use]
pub fn auto_stock_bbox(models: &[LoadedModel]) -> Option<BoundingBox3> {
    let mut boxes = models.iter().filter_map(LoadedModel::bbox);
    let mut union = boxes.next()?;
    for bbox in boxes {
        union.expand_to(bbox.min);
        union.expand_to(bbox.max);
    }
    Some(union)
}

/// Fit `stock` to [`auto_stock_bbox`] of `models` when
/// `stock.auto_from_model` is set. Return `true` when the stock changed.
///
/// The per-box rule is [`StockConfig::update_from_bbox`] (padding, the
/// 2D Z frame). A user-set stock (`auto_from_model` false) never changes.
/// A stock with no model to fit keeps its values. A caller that holds
/// results drops them when this returns `true` (G-FRESHSTATE).
pub(crate) fn fit_auto_stock(stock: &mut StockConfig, models: &[LoadedModel]) -> bool {
    if !stock.auto_from_model {
        return false;
    }
    let Some(bbox) = auto_stock_bbox(models) else {
        return false;
    };
    let before = stock.clone();
    stock.update_from_bbox(&bbox);
    *stock != before
}
