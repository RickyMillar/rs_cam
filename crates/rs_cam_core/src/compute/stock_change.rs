//! S2 (`planning/stock_additions_2026-10-09/PLAN.md`, "The model"): a stock
//! change on a setup.
//!
//! A setup can change the stock before its first toolpath. A change adds
//! material or removes material. The geometry comes from a model or from 2D
//! outlines, and the material is any material from the library. The model
//! is generic: no name, default or text here names one material.
//!
//! `SetupData::stock_changes` holds the list, and the simulation applies it
//! in order, before the setup's first toolpath. Every value is in the SETUP
//! frame: the setup's +Z is "up".
//!
//! This file holds the record, the refusal, the digest that the caches
//! compare, and the resolved form that the simulation request carries
//! ([`ResolvedStockChange`]). S3 applies the resolved form to the stock;
//! S2 only carries it.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ids::{ModelId, StockChangeId};
use crate::material::Material;
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;
pub use crate::stock::cut_as::CutAs;

/// What a stock change does with its geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockChangeOp {
    /// Union the geometry into the stock, as the change's material. The
    /// geometry may rise above the current stock top.
    Add,
    /// Subtract the geometry from every material. The change's material
    /// has no effect.
    Remove,
}

impl StockChangeOp {
    /// The short word that a surface prints: `add` or `remove`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
        }
    }
}

/// Where the volume of a stock change comes from. Values are in the setup
/// frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StockGeometry {
    /// A closed mesh, in this setup's frame.
    Model {
        /// The mesh model.
        model_id: ModelId,
    },
    /// Fill every space open to the top (this setup's +Z), inside the
    /// closed outlines, up to `level_z`. This is what a pour does: it
    /// fills what was CUT, an overcut included.
    OutlineFill {
        /// The 2D models whose closed outlines bound the fill.
        model_ids: Vec<ModelId>,
        /// The fill level, in the setup frame (mm).
        level_z: f64,
    },
    /// A prism from closed 2D outlines, from `z_bottom` to `z_top`.
    OutlineExtrude {
        /// The 2D models whose closed outlines give the prism section.
        model_ids: Vec<ModelId>,
        /// The prism bottom, in the setup frame (mm).
        z_bottom: f64,
        /// The prism top, in the setup frame (mm).
        z_top: f64,
    },
}

impl StockGeometry {
    /// Every model this geometry reads, in declaration order.
    #[must_use]
    pub fn model_ids(&self) -> Vec<ModelId> {
        match self {
            Self::Model { model_id } => vec![*model_id],
            Self::OutlineFill { model_ids, .. } | Self::OutlineExtrude { model_ids, .. } => {
                model_ids.clone()
            }
        }
    }

    /// Does this geometry read `model_id`?
    #[must_use]
    pub fn reads_model(&self, model_id: ModelId) -> bool {
        self.model_ids().contains(&model_id)
    }

    /// The short kind word that a refusal and a surface print.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Model { .. } => "model",
            Self::OutlineFill { .. } => "outline fill",
            Self::OutlineExtrude { .. } => "outline extrude",
        }
    }
}

/// One stock change on a setup.
///
/// The record derives `PartialEq` for the same reason `Fixture` does: a
/// panel edits a clone and must not apply a command for an edit that moved
/// nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockChange {
    /// The id, unique within the setup.
    pub id: StockChangeId,
    /// The name the operator gave. Not an input of the simulation.
    #[serde(default)]
    pub name: String,
    /// A disabled change has no effect on the stock.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Add or remove.
    pub op: StockChangeOp,
    /// Where the volume comes from.
    pub geometry: StockGeometry,
    /// The material that an `Add` puts into the stock: a material from the
    /// library. A `Remove` ignores it.
    #[serde(default)]
    pub material: Material,
    /// S5: the colour that the stock views draw this change's material in,
    /// as 8-bit sRGB. `None` takes the default colour of the material's
    /// slot (`export::material_colour::default_slot_colour`). A display
    /// value only: it is not in [`Self::effect_digest`], so an edit keeps
    /// the simulation. When two changes add the same material, the first
    /// change (in application order) with a colour sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_colour: Option<[u8; 3]>,
    /// S6: how the gates, the cut metrics and the feed modulation cut the
    /// material of an `Add`. The default cuts it as the stock material
    /// (operator ruling 2026-10-09: "in theory it is just more stock"). A
    /// `Remove` ignores it, as it ignores the material.
    #[serde(default)]
    pub cut_as: CutAs,
}

fn default_true() -> bool {
    true
}

impl StockChange {
    /// A digest of every field that decides what the change does to the
    /// stock. The id, the name and the enabled flag are not in it: the
    /// caller keys the id beside the digest, and a disabled change is not
    /// listed at all. A `Remove` ignores its material and its `cut_as`, so
    /// neither is in a `Remove` digest. An `Add` digest holds `cut_as`
    /// (S6): it changes the gate verdicts and the modulated feeds.
    ///
    /// `DefaultHasher` with fixed keys: equal within one build, not across
    /// releases. The material and the geometry hash through their `Debug`
    /// text, the idiom `sim_prefix` uses for a type with no `Hash`. `Debug`
    /// prints an `f64` so that it parses back to the same bits.
    #[must_use]
    pub fn effect_digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.op.hash(&mut hasher);
        format!("{:?}", self.geometry).hash(&mut hasher);
        if self.op == StockChangeOp::Add {
            format!("{:?}", self.material).hash(&mut hasher);
            self.cut_as.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// The effect of a setup's stock-change list: the id and the digest of each
/// ENABLED change, in order.
///
/// Two lists with equal effects change the stock in the same way, for the
/// same model geometry. The mutation doors compare it before and after an
/// edit, so a rename or an edit to a change that stays disabled drops
/// nothing.
#[must_use]
pub fn stock_change_effects(changes: &[StockChange]) -> Vec<(StockChangeId, u64)> {
    changes
        .iter()
        .filter(|c| c.enabled)
        .map(|c| (c.id, c.effect_digest()))
        .collect()
}

/// The id for a new stock change in a setup: one more than the largest id
/// the setup holds, or 0 for an empty list.
///
/// The one allocator. The GUI and MCP both call it, so the two surfaces
/// give a new change the same id.
#[must_use]
pub fn next_stock_change_id(changes: &[StockChange]) -> StockChangeId {
    StockChangeId(changes.iter().map(|c| c.id.0 + 1).max().unwrap_or(0))
}

/// Why a stock change was refused. Each variant names the rule it broke.
#[derive(Debug, Clone, PartialEq)]
pub enum StockChangeRefusal {
    /// The geometry names a model that the project does not hold.
    UnknownModel {
        /// The missing id.
        model_id: ModelId,
    },
    /// A `Model` geometry names a model with no mesh.
    ModelIsNotAMesh {
        /// The model id.
        model_id: ModelId,
        /// The model name.
        name: String,
    },
    /// An outline geometry names a model with no closed 2D outline.
    ModelHasNoClosedOutline {
        /// The model id.
        model_id: ModelId,
        /// The model name.
        name: String,
    },
    /// An outline geometry names no model.
    NoOutlineModels,
    /// A Z value is NaN or infinite.
    ZNotFinite {
        /// The field name.
        field: &'static str,
    },
    /// `OutlineExtrude` with `z_bottom >= z_top`.
    EmptyZRange {
        /// The prism bottom.
        z_bottom: f64,
        /// The prism top.
        z_top: f64,
    },
    /// A `Remove` with an `OutlineFill` geometry. A fill describes the empty
    /// space open to the top, so it holds no material to remove.
    RemoveCannotUseOutlineFill,
    /// Two changes in one setup carry the same id.
    DuplicateId {
        /// The id.
        id: StockChangeId,
    },
}

impl std::fmt::Display for StockChangeRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownModel { model_id } => write!(
                f,
                "the stock change names model id {}, and the project has no such model",
                model_id.0
            ),
            Self::ModelIsNotAMesh { model_id, name } => write!(
                f,
                "a model stock change needs a mesh, and model '{name}' (id {}) has no mesh",
                model_id.0
            ),
            Self::ModelHasNoClosedOutline { model_id, name } => write!(
                f,
                "an outline stock change needs closed 2D outlines, and model '{name}' \
                 (id {}) has no closed outline",
                model_id.0
            ),
            Self::NoOutlineModels => {
                write!(
                    f,
                    "an outline stock change needs at least one outline model"
                )
            }
            Self::ZNotFinite { field } => {
                write!(f, "the stock change field {field} is not a finite number")
            }
            Self::EmptyZRange { z_bottom, z_top } => write!(
                f,
                "an outline extrude needs z_bottom below z_top, and it has \
                 z_bottom {z_bottom} and z_top {z_top}"
            ),
            Self::RemoveCannotUseOutlineFill => write!(
                f,
                "a remove cannot use an outline fill: a fill describes empty space and \
                 holds no material to remove; use an outline extrude or a model"
            ),
            Self::DuplicateId { id } => {
                write!(f, "the setup already holds a stock change with id {}", id.0)
            }
        }
    }
}

impl std::error::Error for StockChangeRefusal {}

/// The geometry facts about one model that the validation reads.
#[derive(Debug, Clone, Copy)]
pub struct ModelGeometryFacts<'a> {
    /// The model name, for the refusal text.
    pub name: &'a str,
    /// The model holds a mesh.
    pub has_mesh: bool,
    /// The model holds at least one closed 2D outline.
    pub has_closed_outline: bool,
}

/// Is `polygon` a closed outline that an outline geometry can use?
///
/// The rule the "Model Outline" boundary applies
/// (`ProjectSession::resolve_model_outline_polys`): a closed polygon with
/// three or more exterior points.
#[must_use]
pub fn is_closed_outline(polygon: &Polygon2) -> bool {
    polygon.closed && polygon.exterior.len() >= 3
}

/// Check one stock change against the project models.
///
/// `facts` answers `None` for an id the project does not hold. The Z values
/// are not compared with the stock: an `Add` may rise above the stock top
/// (the overfill case).
///
/// # Errors
/// The [`StockChangeRefusal`] that names the first rule the change breaks.
pub fn validate_stock_change<'a>(
    change: &StockChange,
    facts: impl Fn(ModelId) -> Option<ModelGeometryFacts<'a>>,
) -> Result<(), StockChangeRefusal> {
    match &change.geometry {
        StockGeometry::Model { model_id } => {
            let model = facts(*model_id).ok_or(StockChangeRefusal::UnknownModel {
                model_id: *model_id,
            })?;
            if !model.has_mesh {
                return Err(StockChangeRefusal::ModelIsNotAMesh {
                    model_id: *model_id,
                    name: model.name.to_owned(),
                });
            }
        }
        StockGeometry::OutlineFill { model_ids, level_z } => {
            if change.op == StockChangeOp::Remove {
                return Err(StockChangeRefusal::RemoveCannotUseOutlineFill);
            }
            check_outline_models(model_ids, &facts)?;
            if !level_z.is_finite() {
                return Err(StockChangeRefusal::ZNotFinite { field: "level_z" });
            }
        }
        StockGeometry::OutlineExtrude {
            model_ids,
            z_bottom,
            z_top,
        } => {
            check_outline_models(model_ids, &facts)?;
            if !z_bottom.is_finite() {
                return Err(StockChangeRefusal::ZNotFinite { field: "z_bottom" });
            }
            if !z_top.is_finite() {
                return Err(StockChangeRefusal::ZNotFinite { field: "z_top" });
            }
            if z_bottom >= z_top {
                return Err(StockChangeRefusal::EmptyZRange {
                    z_bottom: *z_bottom,
                    z_top: *z_top,
                });
            }
        }
    }
    Ok(())
}

fn check_outline_models<'a>(
    model_ids: &[ModelId],
    facts: &impl Fn(ModelId) -> Option<ModelGeometryFacts<'a>>,
) -> Result<(), StockChangeRefusal> {
    if model_ids.is_empty() {
        return Err(StockChangeRefusal::NoOutlineModels);
    }
    for &model_id in model_ids {
        let model = facts(model_id).ok_or(StockChangeRefusal::UnknownModel { model_id })?;
        if !model.has_closed_outline {
            return Err(StockChangeRefusal::ModelHasNoClosedOutline {
                model_id,
                name: model.name.to_owned(),
            });
        }
    }
    Ok(())
}

/// The geometry source of one model that a resolved stock change reads.
///
/// The `Arc` is the one the session holds, so the S5 prefix memo can key
/// the source by pointer identity, as it keys the model mesh: a model
/// refresh swaps the `Arc`, and the memo misses.
#[derive(Debug, Clone)]
pub enum StockChangeSource {
    /// The model mesh (a `Model` geometry).
    Mesh(Arc<TriangleMesh>),
    /// The model's 2D polygons (an outline geometry). Open paths are in
    /// the list; the S3 kernel takes the closed ones only
    /// ([`is_closed_outline`]).
    Outlines(Arc<Vec<Polygon2>>),
    /// The model is absent or holds no geometry of the kind the change
    /// needs. The validation refuses this case when the change is written;
    /// a later model removal or a failed reload can still leave it.
    Missing(ModelId),
}

/// One ENABLED stock change of a simulation group, with the model geometry
/// it reads.
///
/// [`crate::compute::simulate::SimGroupEntry::stock_changes`] carries a list
/// of these, in application order. The geometry is in the MODEL's frame, as
/// the session holds it; S3 maps it into the setup frame.
#[derive(Debug, Clone)]
pub struct ResolvedStockChange {
    /// The `SetupData::id` of the setup that owns the change.
    pub setup_id: usize,
    /// The change record.
    pub change: StockChange,
    /// One source per model in `change.geometry.model_ids()`, in order.
    pub sources: Vec<StockChangeSource>,
}

impl ResolvedStockChange {
    /// The digest that the caches compare: [`StockChange::effect_digest`].
    #[must_use]
    pub fn effect_digest(&self) -> u64 {
        self.change.effect_digest()
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn fill(level_z: f64) -> StockChange {
        StockChange {
            id: StockChangeId(0),
            name: "Fill".to_owned(),
            enabled: true,
            op: StockChangeOp::Add,
            geometry: StockGeometry::OutlineFill {
                model_ids: vec![ModelId(3)],
                level_z,
            },
            material: Material::Custom {
                name: "Resin".to_owned(),
                feed_scale_factor: 1.0,
            },
            display_colour: None,
            cut_as: CutAs::StockMaterial,
        }
    }

    fn outline_facts(id: ModelId) -> Option<ModelGeometryFacts<'static>> {
        (id == ModelId(3)).then_some(ModelGeometryFacts {
            name: "outline",
            has_mesh: false,
            has_closed_outline: true,
        })
    }

    #[test]
    fn the_digest_ignores_the_name_and_a_remove_material() {
        let a = fill(1.0);
        let mut renamed = a.clone();
        renamed.name = "Other".to_owned();
        assert_eq!(a.effect_digest(), renamed.effect_digest());

        let mut moved = a.clone();
        moved.geometry = StockGeometry::OutlineFill {
            model_ids: vec![ModelId(3)],
            level_z: 1.5,
        };
        assert_ne!(a.effect_digest(), moved.effect_digest());

        let mut remove = a.clone();
        remove.op = StockChangeOp::Remove;
        let mut remove_other_material = remove.clone();
        remove_other_material.material = Material::default();
        assert_eq!(
            remove.effect_digest(),
            remove_other_material.effect_digest()
        );
        let mut add_other_material = a.clone();
        add_other_material.material = Material::default();
        assert_ne!(a.effect_digest(), add_other_material.effect_digest());

        // S6: `cut_as` changes an `Add` digest and not a `Remove` digest.
        let mut add_own = a.clone();
        add_own.cut_as = CutAs::OwnMaterial;
        assert_ne!(a.effect_digest(), add_own.effect_digest());
        let mut remove_own = remove.clone();
        remove_own.cut_as = CutAs::OwnMaterial;
        assert_eq!(remove.effect_digest(), remove_own.effect_digest());
    }

    #[test]
    fn a_remove_with_an_outline_fill_is_refused_by_name() {
        let mut removal = fill(1.0);
        removal.op = StockChangeOp::Remove;
        assert_eq!(
            validate_stock_change(&removal, outline_facts),
            Err(StockChangeRefusal::RemoveCannotUseOutlineFill)
        );
        removal.geometry = StockGeometry::OutlineExtrude {
            model_ids: vec![ModelId(3)],
            z_bottom: 0.0,
            z_top: 1.0,
        };
        assert_eq!(validate_stock_change(&removal, outline_facts), Ok(()));
    }

    #[test]
    fn the_next_id_is_one_more_than_the_largest() {
        assert_eq!(next_stock_change_id(&[]), StockChangeId(0));
        let mut late = fill(1.0);
        late.id = StockChangeId(4);
        assert_eq!(next_stock_change_id(&[fill(1.0), late]), StockChangeId(5));
    }

    #[test]
    fn the_effects_list_only_enabled_changes() {
        let mut off = fill(1.0);
        off.id = StockChangeId(1);
        off.enabled = false;
        let effects = stock_change_effects(&[fill(1.0), off]);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].0, StockChangeId(0));
    }

    #[test]
    fn the_validation_names_each_rule() {
        assert_eq!(validate_stock_change(&fill(1.0), outline_facts), Ok(()));
        assert_eq!(
            validate_stock_change(&fill(f64::NAN), outline_facts),
            Err(StockChangeRefusal::ZNotFinite { field: "level_z" })
        );
        let mut mesh = fill(1.0);
        mesh.geometry = StockGeometry::Model {
            model_id: ModelId(3),
        };
        assert!(matches!(
            validate_stock_change(&mesh, outline_facts),
            Err(StockChangeRefusal::ModelIsNotAMesh { .. })
        ));
        let mut empty = fill(1.0);
        empty.geometry = StockGeometry::OutlineExtrude {
            model_ids: vec![ModelId(3)],
            z_bottom: 2.0,
            z_top: 2.0,
        };
        assert!(matches!(
            validate_stock_change(&empty, outline_facts),
            Err(StockChangeRefusal::EmptyZRange { .. })
        ));
        let mut none = fill(1.0);
        none.geometry = StockGeometry::OutlineFill {
            model_ids: Vec::new(),
            level_z: 1.0,
        };
        assert_eq!(
            validate_stock_change(&none, outline_facts),
            Err(StockChangeRefusal::NoOutlineModels)
        );
        let mut unknown = fill(1.0);
        unknown.geometry = StockGeometry::OutlineFill {
            model_ids: vec![ModelId(9)],
            level_z: 1.0,
        };
        assert_eq!(
            validate_stock_change(&unknown, outline_facts),
            Err(StockChangeRefusal::UnknownModel {
                model_id: ModelId(9)
            })
        );
    }
}
