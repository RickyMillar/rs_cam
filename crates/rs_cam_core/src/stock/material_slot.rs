//! S1 (`planning/stock_additions_2026-10-09/PLAN.md`): the material of each
//! dexel segment.
//!
//! Each [`crate::stock::dexel::DexelSegment`] carries a [`MaterialSlot`].
//! Slot 0 ([`MaterialSlot::STOCK`]) is the stock's own material, which
//! `StockConfig.material` names. A slot `k > 0` is an ADDED material: entry
//! `k - 1` of the stock's [`MaterialSlotTable`]. The table holds a
//! [`Material`] value from the material library. No material is special.
//!
//! The stamp kernels record the removal per slot in a [`SlotTally`] beside
//! their own volume sums, and each cut sample keeps the result as a
//! [`MaterialCut`]:
//! the slot that the tool removed most of (by volume) and a flag when the
//! tool removed more than one material.

use serde::{Deserialize, Serialize};

use crate::material::Material;

/// The number of slots a stock can hold, slot 0 included. The coalescer's
/// [`SlotVolumes`] holds one `f64` per slot, so this value sets its size.
/// [`MaterialSlotTable::slot_for`] refuses a material past it.
pub const MATERIAL_SLOT_CAPACITY: usize = 8;

/// The material of one dexel segment: an index into the stock's
/// [`MaterialSlotTable`]. One byte, so a segment stays 12 bytes and a
/// one-segment ray stays inline (`size_of::<DexelRay>() == 24`).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct MaterialSlot(pub u8);

impl MaterialSlot {
    /// The stock's own material (`StockConfig.material`).
    pub const STOCK: Self = Self(0);

    /// `true` for the stock's own material.
    #[inline]
    #[must_use]
    pub fn is_stock(self) -> bool {
        self.0 == 0
    }

    /// The slot as an index into a per-slot array of length
    /// [`MATERIAL_SLOT_CAPACITY`].
    #[inline]
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// The table refuses a new material: every slot is in use.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialSlotsFull {
    /// The material that did not get a slot.
    pub material: Material,
}

impl std::fmt::Display for MaterialSlotsFull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the stock holds {} added materials, the maximum; '{}' gets no slot",
            MATERIAL_SLOT_CAPACITY - 1,
            self.material.label()
        )
    }
}

impl std::error::Error for MaterialSlotsFull {}

/// Slot -> material for the added materials of one stock.
///
/// Slot 0 is not in the table: it is the stock's own material, and the
/// caller that owns `StockConfig` resolves it ([`Self::material`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialSlotTable {
    /// Entry `k - 1` is the material of slot `k`.
    added: Vec<Material>,
}

impl MaterialSlotTable {
    /// `true` when the stock holds only its own material.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
    }

    /// The slot of `material`. An equal material that is already in the
    /// table keeps its slot; a new one gets the next free slot.
    ///
    /// The stock's own material is not compared: a caller that adds the
    /// stock material again and wants slot 0 passes [`MaterialSlot::STOCK`]
    /// itself.
    ///
    /// # Errors
    ///
    /// [`MaterialSlotsFull`] when every slot up to
    /// [`MATERIAL_SLOT_CAPACITY`] is in use.
    pub fn slot_for(&mut self, material: &Material) -> Result<MaterialSlot, MaterialSlotsFull> {
        if let Some(i) = self.added.iter().position(|m| m == material) {
            return Ok(MaterialSlot((i + 1) as u8));
        }
        if self.added.len() + 1 >= MATERIAL_SLOT_CAPACITY {
            return Err(MaterialSlotsFull {
                material: material.clone(),
            });
        }
        self.added.push(material.clone());
        Ok(MaterialSlot(self.added.len() as u8))
    }

    /// The material of `slot`. Slot 0 gives `stock_material`. A slot that is
    /// not in the table gives `None`.
    #[must_use]
    pub fn material<'a>(
        &'a self,
        slot: MaterialSlot,
        stock_material: &'a Material,
    ) -> Option<&'a Material> {
        match slot.index() {
            0 => Some(stock_material),
            k => self.added.get(k - 1),
        }
    }

    /// The added materials in slot order (slot 1 first).
    pub fn added(&self) -> impl Iterator<Item = (MaterialSlot, &Material)> {
        self.added
            .iter()
            .enumerate()
            .map(|(i, m)| (MaterialSlot((i + 1) as u8), m))
    }
}

/// Removed volume per slot, in mm³. The sample coalescer sums each sample's
/// removed volume under its main slot here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlotVolumes(pub [f64; MATERIAL_SLOT_CAPACITY]);

impl SlotVolumes {
    /// Add `volume` to `slot`. A slot past the capacity cannot exist
    /// ([`MaterialSlotTable::slot_for`] refuses it); its volume goes to the
    /// last slot so that the total stays true.
    #[inline]
    pub fn add(&mut self, slot: MaterialSlot, volume: f64) {
        let i = slot.index().min(MATERIAL_SLOT_CAPACITY - 1);
        if let Some(v) = self.0.get_mut(i) {
            *v += volume;
        }
    }

    /// Fold another accumulator into this one.
    #[inline]
    pub fn merge(&mut self, other: &Self) {
        for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
            *a += b;
        }
    }

    /// The main slot by removed volume, and the flag "more than one slot
    /// lost material". An accumulator with no volume gives the stock slot.
    #[must_use]
    pub fn material_cut(&self) -> MaterialCut {
        let mut main = MaterialSlot::STOCK;
        let mut main_volume = 0.0_f64;
        let mut removed_slots = 0_usize;
        for (i, &v) in self.0.iter().enumerate() {
            if v > 0.0 {
                removed_slots += 1;
                if v > main_volume {
                    main_volume = v;
                    main = MaterialSlot(i as u8);
                }
            }
        }
        MaterialCut {
            slot: main,
            several: removed_slots > 1,
        }
    }
}

/// The per-stamp material record of the metric kernels: 6 bytes with
/// alignment 1, so it fits in the tail padding of `StampPartial` and the
/// documented batch bound (80 B per partial) holds. The volume is an `f32`
/// kept as its bytes for that reason.
///
/// It keeps the volume of the ADDED material only. The stock part is the
/// stamp's own removed volume minus it ([`Self::material_cut`]). Two flags
/// record what a volume difference cannot: a stock segment lost length, and
/// two different added slots lost length. With two added slots in one stamp,
/// `added_slot` is the first one seen and the flag says so.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlotTally {
    added_mm3: [u8; 4],
    added_slot: MaterialSlot,
    flags: u8,
}

impl SlotTally {
    const STOCK_CUT: u8 = 1;
    const SEVERAL_ADDED: u8 = 2;

    #[inline]
    fn added(&self) -> f32 {
        f32::from_ne_bytes(self.added_mm3)
    }

    /// Record `volume` (mm³) removed from `slot`.
    #[inline]
    pub fn add(&mut self, slot: MaterialSlot, volume: f64) {
        if volume <= 0.0 {
            return;
        }
        if slot.is_stock() {
            self.flags |= Self::STOCK_CUT;
            return;
        }
        self.add_added(slot, volume as f32);
    }

    #[inline]
    fn add_added(&mut self, slot: MaterialSlot, volume: f32) {
        let added = self.added();
        if added > 0.0 {
            if slot != self.added_slot {
                self.flags |= Self::SEVERAL_ADDED;
            }
        } else {
            self.added_slot = slot;
        }
        self.added_mm3 = (added + volume).to_ne_bytes();
    }

    /// Fold another tally into this one.
    #[inline]
    pub fn merge(&mut self, other: &Self) {
        self.flags |= other.flags;
        let volume = other.added();
        if volume > 0.0 {
            self.add_added(other.added_slot, volume);
        }
    }

    /// This tally with its volume scaled by `weight` (a swept bin's share).
    #[inline]
    #[must_use]
    pub fn scaled(&self, weight: f64) -> Self {
        Self {
            added_mm3: ((f64::from(self.added()) * weight) as f32).to_ne_bytes(),
            ..*self
        }
    }

    /// The material cut of a stamp that removed `removed_mm3` in total.
    #[must_use]
    pub fn material_cut(&self, removed_mm3: f64) -> MaterialCut {
        let added = f64::from(self.added());
        if added <= 0.0 {
            return MaterialCut::default();
        }
        let stock_part = removed_mm3 - added;
        MaterialCut {
            slot: if added > stock_part {
                self.added_slot
            } else {
                MaterialSlot::STOCK
            },
            several: self.flags != 0,
        }
    }
}

/// What material one cut sample removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaterialCut {
    /// The slot that lost the most volume.
    pub slot: MaterialSlot,
    /// More than one slot lost material.
    pub several: bool,
}

/// One per-segment walk: the volume that removing everything above
/// `surface`, scaled by `coverage`, takes from each slot. It is the per-slot
/// share of what `ray_blend_above(ray, surface, coverage)` removes, so call
/// it BEFORE the blend.
#[inline]
pub fn tally_blend_above(
    ray: &[crate::stock::dexel::DexelSegment],
    surface: f32,
    coverage: f32,
    cell_area: f64,
    out: &mut SlotTally,
) {
    let f = f64::from(coverage.clamp(0.0, 1.0));
    for seg in ray {
        let lo = seg.enter.max(surface);
        if seg.exit > lo {
            out.add(seg.material, f * f64::from(seg.exit - lo) * cell_area);
        }
    }
}

/// The mirror of [`tally_blend_above`] for `ray_blend_below`.
#[inline]
pub fn tally_blend_below(
    ray: &[crate::stock::dexel::DexelSegment],
    surface: f32,
    coverage: f32,
    cell_area: f64,
    out: &mut SlotTally,
) {
    let f = f64::from(coverage.clamp(0.0, 1.0));
    for seg in ray {
        let hi = seg.exit.min(surface);
        if hi > seg.enter {
            out.add(seg.material, f * f64::from(hi - seg.enter) * cell_area);
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::mem::size_of;

    fn fill_material(name: &str) -> Material {
        Material::Custom {
            name: name.to_owned(),
            feed_scale_factor: 1.0,
        }
    }

    #[test]
    fn the_table_gives_one_slot_per_material_and_refuses_past_the_capacity() {
        let mut table = MaterialSlotTable::default();
        let a = table.slot_for(&fill_material("a")).unwrap();
        let b = table.slot_for(&fill_material("b")).unwrap();
        assert_eq!(a, MaterialSlot(1));
        assert_eq!(b, MaterialSlot(2));
        assert_eq!(table.slot_for(&fill_material("a")).unwrap(), a);
        let stock = Material::default();
        assert_eq!(table.material(MaterialSlot::STOCK, &stock), Some(&stock));
        assert_eq!(table.material(b, &stock), Some(&fill_material("b")));
        assert_eq!(table.material(MaterialSlot(7), &stock), None);
        for k in 3..MATERIAL_SLOT_CAPACITY {
            table.slot_for(&fill_material(&format!("m{k}"))).unwrap();
        }
        assert!(table.slot_for(&fill_material("one too many")).is_err());
    }

    #[test]
    fn the_stamp_tally_names_the_main_slot_and_flags_two_materials() {
        let mut t = SlotTally::default();
        assert_eq!(t.material_cut(5.0), MaterialCut::default());
        // Only the added material: main slot 1, no flag.
        t.add(MaterialSlot(1), 2.0);
        assert_eq!(
            t.material_cut(2.0),
            MaterialCut {
                slot: MaterialSlot(1),
                several: false
            }
        );
        // The stock loses more than the added material.
        let mut mixed = t;
        mixed.add(MaterialSlot::STOCK, 3.0);
        assert_eq!(
            mixed.material_cut(5.0),
            MaterialCut {
                slot: MaterialSlot::STOCK,
                several: true
            }
        );
        // Two added slots: the flag, and the volume of both.
        let mut other = SlotTally::default();
        other.add(MaterialSlot(2), 1.0);
        let mut both = t;
        both.merge(&other);
        assert_eq!(
            both.material_cut(3.0),
            MaterialCut {
                slot: MaterialSlot(1),
                several: true
            }
        );
        assert_eq!(t.scaled(0.5).material_cut(1.0).slot, MaterialSlot(1));
        assert_eq!(size_of::<SlotTally>(), 6);
        assert_eq!(std::mem::align_of::<SlotTally>(), 1);
    }

    #[test]
    fn the_main_slot_is_the_one_with_the_most_volume() {
        let mut v = SlotVolumes::default();
        assert_eq!(v.material_cut(), MaterialCut::default());
        v.add(MaterialSlot(2), 1.0);
        assert_eq!(
            v.material_cut(),
            MaterialCut {
                slot: MaterialSlot(2),
                several: false
            }
        );
        v.add(MaterialSlot::STOCK, 3.0);
        assert_eq!(
            v.material_cut(),
            MaterialCut {
                slot: MaterialSlot::STOCK,
                several: true
            }
        );
    }
}
