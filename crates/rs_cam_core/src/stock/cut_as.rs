//! S6 (`planning/stock_additions_2026-10-09/PLAN.md`): how the gates, the
//! cut metrics and the feed modulation cut an ADDED material.
//!
//! Operator ruling (2026-10-09): "in theory it is just more stock". So an
//! added material is cut as the stock material by default
//! ([`CutAs::StockMaterial`]). A stock change can ask for
//! [`CutAs::OwnMaterial`]: then the samples whose main slot is that
//! material use its own force data from the library. A material with no
//! force data is "not judged".
//!
//! The trace records, per added slot, the material and its [`CutAs`]
//! ([`AddedMaterialSlot`]). Every gate and the modulation read one helper,
//! [`SimulationCutTrace::effective_material_for_sample`]. Toolpath
//! generation and Suggest never read the added material.

use serde::{Deserialize, Serialize};

use crate::ids::ToolpathId;
use crate::material::Material;
use crate::stock::material_slot::MaterialSlot;
use crate::stock::simulation_cut::{SimulationCutSample, SimulationCutTrace};

/// How the gates, the cut metrics and the feed modulation cut the material
/// of an `Add` stock change. A `Remove` ignores it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutAs {
    /// The added material is cut exactly as the stock material: the same
    /// force data, the same chip-load band. A mixed cut is a normal stock
    /// cut. The default (operator ruling 2026-10-09).
    #[default]
    StockMaterial,
    /// The added material uses its own force data from the library. With
    /// no force data, its samples are "not judged".
    OwnMaterial,
}

impl CutAs {
    /// The text that every surface prints: `cut as stock` or
    /// `own material`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::StockMaterial => "cut as stock",
            Self::OwnMaterial => "own material",
        }
    }

    /// The wire token: `stock_material` or `own_material`.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::StockMaterial => "stock_material",
            Self::OwnMaterial => "own_material",
        }
    }
}

/// One added slot of the simulated stock: the material that a stock change
/// wrote into it, and how the gates cut it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddedMaterialSlot {
    /// The slot (never [`MaterialSlot::STOCK`]).
    pub slot: MaterialSlot,
    /// The material of the slot.
    pub material: Material,
    /// How the gates cut the slot.
    pub cut_as: CutAs,
}

/// Why a sample is not judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotJudged {
    /// The label of the material that has no force data.
    pub material: String,
}

impl std::fmt::Display for NotJudged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not judged: no force data for {}", self.material)
    }
}

/// The material that the gates use for one sample.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectiveMaterial<'a> {
    /// Judge the sample as this material.
    Judged(&'a Material),
    /// Do not judge the sample.
    NotJudged(NotJudged),
}

impl SimulationCutTrace {
    /// `true` when at least one added slot asks for its own material. When
    /// this is `false`, every sample is judged as the stock material.
    #[must_use]
    pub fn has_own_material_slots(&self) -> bool {
        self.added_material_slots
            .iter()
            .any(|s| s.cut_as == CutAs::OwnMaterial)
    }

    /// **The one helper**: the material that the gates, the cut-metric
    /// histograms and the feed modulation use for `sample`.
    ///
    /// - Slot 0 (the stock), a slot that the trace does not list, and a
    ///   slot with [`CutAs::StockMaterial`] give `Judged(stock)`.
    /// - A slot with [`CutAs::OwnMaterial`] gives `Judged(its material)`
    ///   when the material has force data (`Material::force_line`), else
    ///   `NotJudged`.
    ///
    /// A sample that cut several materials is judged by its main slot
    /// (`material_slot`); `cuts_several_materials` is not read. Under the
    /// default, a mixed cut is therefore a normal stock cut.
    #[must_use]
    pub fn effective_material_for_sample<'a>(
        &'a self,
        sample: &SimulationCutSample,
        stock: &'a Material,
    ) -> EffectiveMaterial<'a> {
        self.effective_material_for_slot(sample.material_slot, stock)
    }

    /// [`Self::effective_material_for_sample`] for a slot.
    #[must_use]
    pub fn effective_material_for_slot<'a>(
        &'a self,
        slot: MaterialSlot,
        stock: &'a Material,
    ) -> EffectiveMaterial<'a> {
        if slot.is_stock() {
            return EffectiveMaterial::Judged(stock);
        }
        let Some(added) = self.added_material_slots.iter().find(|s| s.slot == slot) else {
            return EffectiveMaterial::Judged(stock);
        };
        match added.cut_as {
            CutAs::StockMaterial => EffectiveMaterial::Judged(stock),
            CutAs::OwnMaterial => {
                if added.material.force_line().is_ok() {
                    EffectiveMaterial::Judged(&added.material)
                } else {
                    EffectiveMaterial::NotJudged(NotJudged {
                        material: added.material.label(),
                    })
                }
            }
        }
    }

    /// The material populations of one toolpath: each judged material
    /// other than `stock` with its sample count, and each not-judged
    /// material with its sample count. Both lists are empty when every
    /// sample of the toolpath is judged as `stock` (always so under the
    /// default).
    #[must_use]
    pub fn material_populations<'a>(
        &'a self,
        toolpath_id: ToolpathId,
        stock: &'a Material,
    ) -> MaterialPopulations<'a> {
        let mut out = MaterialPopulations::default();
        if !self.has_own_material_slots() {
            return out;
        }
        for s in self.samples.iter().filter(|s| s.toolpath_id == toolpath_id) {
            match self.effective_material_for_sample(s, stock) {
                EffectiveMaterial::Judged(m) if m == stock => {}
                EffectiveMaterial::Judged(m) => {
                    match out.own.iter_mut().find(|(own, _)| *own == m) {
                        Some((_, n)) => *n += 1,
                        None => out.own.push((m, 1)),
                    }
                }
                EffectiveMaterial::NotJudged(reason) => {
                    match out
                        .not_judged
                        .iter_mut()
                        .find(|c| c.material == reason.material)
                    {
                        Some(c) => c.samples += 1,
                        None => out.not_judged.push(NotJudgedCount {
                            material: reason.material,
                            samples: 1,
                        }),
                    }
                }
            }
        }
        out
    }

    /// A copy of this trace in which only the samples of `toolpath_id` that
    /// `keep` accepts still belong to `toolpath_id`. Every other sample of
    /// that toolpath moves to [`POPULATION_EXCLUDED`], so the gates skip it
    /// and every sample keeps its index (the evidence indices stay true).
    #[must_use]
    pub fn population_view(
        &self,
        toolpath_id: ToolpathId,
        keep: impl Fn(&SimulationCutSample) -> bool,
    ) -> Self {
        let mut view = self.clone();
        for s in view
            .samples
            .iter_mut()
            .filter(|s| s.toolpath_id == toolpath_id)
        {
            if !keep(s) {
                s.toolpath_id = POPULATION_EXCLUDED;
            }
        }
        view
    }

    /// The stock population of `toolpath_id`: the samples judged as
    /// `stock`. `None` when that is every sample of the toolpath (the
    /// default), so the caller reads the trace itself.
    #[must_use]
    pub fn stock_population_view(&self, toolpath_id: ToolpathId, stock: &Material) -> Option<Self> {
        let populations = self.material_populations(toolpath_id, stock);
        if populations.is_empty() {
            return None;
        }
        Some(self.population_view(toolpath_id, |s| self.judged_as(s, stock, stock)))
    }

    /// `true` when the helper judges `sample` as `material`.
    #[must_use]
    pub fn judged_as(
        &self,
        sample: &SimulationCutSample,
        stock: &Material,
        material: &Material,
    ) -> bool {
        matches!(
            self.effective_material_for_sample(sample, stock),
            EffectiveMaterial::Judged(m) if m == material
        )
    }
}

/// The toolpath id that [`SimulationCutTrace::population_view`] gives a
/// sample outside the population. No toolpath has it.
pub const POPULATION_EXCLUDED: ToolpathId = ToolpathId(usize::MAX);

/// The number of samples of one material that the gates did not judge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotJudgedCount {
    /// The material label.
    pub material: String,
    /// The number of samples.
    pub samples: usize,
}

impl NotJudgedCount {
    /// The text that every surface prints, for example
    /// `412 samples not judged: no force data for Epoxy`.
    #[must_use]
    pub fn label(&self) -> String {
        format!(
            "{} samples {}",
            self.samples,
            NotJudged {
                material: self.material.clone()
            }
        )
    }
}

/// The populations of one toolpath that are not the stock population.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialPopulations<'a> {
    /// Each judged own material and its sample count, in first-seen order.
    pub own: Vec<(&'a Material, usize)>,
    /// Each not-judged material and its sample count, in first-seen order.
    pub not_judged: Vec<NotJudgedCount>,
}

impl MaterialPopulations<'_> {
    /// `true` when every sample is in the stock population.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.own.is_empty() && self.not_judged.is_empty()
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
    use crate::material::WoodSpecies;

    fn resin() -> Material {
        Material::Custom {
            name: "Resin".to_owned(),
            feed_scale_factor: 1.0,
        }
    }

    fn trace_with(slots: Vec<AddedMaterialSlot>) -> SimulationCutTrace {
        let mut t = SimulationCutTrace::from_samples(1.0, Vec::new());
        t.added_material_slots = slots;
        t
    }

    #[test]
    fn the_default_judges_an_added_slot_as_the_stock() {
        let stock = Material::default();
        let t = trace_with(vec![AddedMaterialSlot {
            slot: MaterialSlot(1),
            material: resin(),
            cut_as: CutAs::StockMaterial,
        }]);
        assert!(!t.has_own_material_slots());
        assert_eq!(
            t.effective_material_for_slot(MaterialSlot(1), &stock),
            EffectiveMaterial::Judged(&stock)
        );
    }

    #[test]
    fn own_material_reads_the_force_data_or_is_not_judged() {
        let stock = Material::default();
        let maple = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        assert!(maple.force_line().is_ok());
        let t = trace_with(vec![
            AddedMaterialSlot {
                slot: MaterialSlot(1),
                material: resin(),
                cut_as: CutAs::OwnMaterial,
            },
            AddedMaterialSlot {
                slot: MaterialSlot(2),
                material: maple.clone(),
                cut_as: CutAs::OwnMaterial,
            },
        ]);
        assert_eq!(
            t.effective_material_for_slot(MaterialSlot(1), &stock),
            EffectiveMaterial::NotJudged(NotJudged {
                material: "Resin".to_owned()
            })
        );
        assert_eq!(
            t.effective_material_for_slot(MaterialSlot(2), &stock),
            EffectiveMaterial::Judged(&maple)
        );
        assert_eq!(
            t.effective_material_for_slot(MaterialSlot::STOCK, &stock),
            EffectiveMaterial::Judged(&stock)
        );
        assert_eq!(
            NotJudged {
                material: "Resin".to_owned()
            }
            .to_string(),
            "not judged: no force data for Resin"
        );
    }
}
