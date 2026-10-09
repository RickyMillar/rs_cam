//! S5 (`planning/stock_additions_2026-10-09/PLAN.md`): the colour of each
//! stock material in a stock view.
//!
//! The mesh builders bake the wood depth shade into `StockMesh::colors` and
//! record the material slot of each vertex in `StockMesh::material_slots`.
//! This module chooses the colour of a slot:
//!
//! - slot 0 (the stock material) keeps the baked wood shade, unchanged;
//! - a slot `k > 0` takes the colour of its material
//!   ([`MaterialPalette`]), times a lightness term from the baked shade.
//!   The relief thus reads on an added material as it does on the wood.
//!
//! The GPU viewport, the software renderer (`screenshot_simulation`) and the
//! legend read the same palette, so the three show the same colour.

use crate::stock::dexel_mesh::{UNCUT_B, UNCUT_G, UNCUT_R};
use crate::stock::material_slot::{MATERIAL_SLOT_CAPACITY, MaterialSlot};
use crate::stock::stock_mesh::StockMesh;

/// The hue step between two consecutive slots, in degrees.
///
/// The rule: 8 hues at 45 degree spacing, with the wood hue as hue 0. A
/// step of 3 x 45 = 135 degrees visits the 7 other hues once each (3 and 8
/// have no common factor), and it puts consecutive slots far apart on the
/// hue circle.
const HUE_STEP_DEG: f32 = 135.0;

/// The HSL saturation of a default slot colour. The value is a choice for a
/// dark background: saturated enough to differ from the grey user
/// interface, not so saturated that the relief shade is lost.
const DEFAULT_SATURATION: f32 = 0.6;

/// The HSL lightness of a default slot colour (the same choice as
/// [`DEFAULT_SATURATION`]).
const DEFAULT_LIGHTNESS: f32 = 0.55;

/// The Rec. 709 luma weights (ITU-R BT.709, the sRGB primaries).
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// The colour of each material slot of one stock, as linear `[r, g, b]` in
/// `0..=1` (the space of `StockMesh::colors`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialPalette {
    colours: [[f32; 3]; MATERIAL_SLOT_CAPACITY],
}

impl Default for MaterialPalette {
    /// Slot 0 is the uncut wood colour; slot `k` is
    /// [`default_slot_colour`]`(k)`.
    fn default() -> Self {
        let mut colours = [[0.0; 3]; MATERIAL_SLOT_CAPACITY];
        for (k, colour) in colours.iter_mut().enumerate() {
            *colour = default_slot_colour(MaterialSlot(k as u8));
        }
        Self { colours }
    }
}

impl MaterialPalette {
    /// The palette with `slot` drawn in `colour`. Slot 0 keeps the wood
    /// shade in every view, so a colour for slot 0 changes only its legend
    /// swatch.
    #[must_use]
    pub fn with_colour(mut self, slot: MaterialSlot, colour: [f32; 3]) -> Self {
        if let Some(entry) = self.colours.get_mut(slot.index()) {
            *entry = colour;
        }
        self
    }

    /// The colour of `slot` at the uncut surface. A slot past the capacity
    /// gets the slot 0 colour.
    #[must_use]
    pub fn colour(&self, slot: MaterialSlot) -> [f32; 3] {
        self.colours
            .get(slot.index())
            .copied()
            .unwrap_or_else(|| default_slot_colour(MaterialSlot::STOCK))
    }

    /// The colour of a vertex of `slot` whose baked wood shade is `baked`.
    ///
    /// Slot 0 gives `baked` unchanged. Another slot gives its colour times
    /// the luma of `baked` over the luma of the uncut wood: 1 at the uncut
    /// top, and lower with depth as the wood ramp goes darker.
    #[must_use]
    pub fn shade(&self, slot: MaterialSlot, baked: [f32; 3]) -> [f32; 3] {
        if slot.is_stock() {
            return baked;
        }
        let lightness = (luma(baked) / luma([UNCUT_R, UNCUT_G, UNCUT_B])).clamp(0.0, 1.0);
        let [r, g, b] = self.colour(slot);
        [r * lightness, g * lightness, b * lightness]
    }
}

/// The default colour of a slot. Slot 0 is the uncut wood colour. Slot `k`
/// is the HSL colour at hue `wood hue + k x 135` degrees,
/// [`DEFAULT_SATURATION`] and [`DEFAULT_LIGHTNESS`] (see [`HUE_STEP_DEG`]).
#[must_use]
pub fn default_slot_colour(slot: MaterialSlot) -> [f32; 3] {
    if slot.is_stock() {
        return [UNCUT_R, UNCUT_G, UNCUT_B];
    }
    let wood_hue = hue_deg([UNCUT_R, UNCUT_G, UNCUT_B]);
    let hue = (wood_hue + f32::from(slot.0) * HUE_STEP_DEG).rem_euclid(360.0);
    hsl_to_rgb(hue, DEFAULT_SATURATION, DEFAULT_LIGHTNESS)
}

/// An 8-bit sRGB display colour (`StockChange::display_colour`) as the
/// `0..=1` floats the mesh colours use.
#[must_use]
pub fn colour_from_rgb8(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| f32::from(c) / 255.0)
}

/// One colour per vertex of `mesh`: [`MaterialPalette::shade`] of the slot
/// and the baked colour. A mesh with no slot gives its baked colours,
/// unchanged. A vertex with no baked colour takes the uncut wood colour.
#[must_use]
pub fn material_colors(mesh: &StockMesh, palette: &MaterialPalette) -> Vec<[f32; 3]> {
    let (baked, _) = mesh.colors.as_chunks::<3>();
    (0..mesh.vertex_count())
        .map(|i| {
            let colour = baked.get(i).copied().unwrap_or([UNCUT_R, UNCUT_G, UNCUT_B]);
            palette.shade(mesh.slot_at(i), colour)
        })
        .collect()
}

/// Write [`material_colors`] into `mesh.colors`. A mesh with no added
/// material is not changed.
pub fn apply_material_colours(mesh: &mut StockMesh, palette: &MaterialPalette) {
    if !mesh.has_added_material() {
        return;
    }
    let colours = material_colors(mesh, palette);
    mesh.colors = colours.into_iter().flatten().collect();
}

/// The height gradient of the software renderer
/// ([`crate::export::ribbon::apply_height_gradient`]) on the slot 0
/// vertices, and the material colour on every other vertex.
///
/// A mesh with no added material gets the plain height gradient, bit for bit.
pub fn apply_height_gradient_keeping_materials(mesh: &mut StockMesh, palette: &MaterialPalette) {
    let materials = mesh
        .has_added_material()
        .then(|| material_colors(mesh, palette));
    crate::export::ribbon::apply_height_gradient(mesh);
    let Some(materials) = materials else {
        return;
    };
    for (i, colour) in materials.into_iter().enumerate() {
        if mesh.slot_at(i).is_stock() {
            continue;
        }
        if let Some(dst) = mesh.colors.get_mut(i * 3..i * 3 + 3) {
            dst.copy_from_slice(&colour);
        }
    }
}

fn luma(c: [f32; 3]) -> f32 {
    LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2]
}

/// The HSL hue of an RGB colour, in degrees `0..360`.
fn hue_deg([r, g, b]: [f32; 3]) -> f32 {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    if chroma <= 0.0 {
        return 0.0;
    }
    let sector = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    sector * 60.0
}

/// HSL to RGB (the standard sector formula).
fn hsl_to_rgb(hue_deg: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue_deg / 60.0;
    let x = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    [r + m, g + m, b + m]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    #[test]
    fn the_default_palette_gives_each_slot_a_distinct_colour() {
        let palette = MaterialPalette::default();
        let colours: Vec<[f32; 3]> = (0..MATERIAL_SLOT_CAPACITY)
            .map(|k| palette.colour(MaterialSlot(k as u8)))
            .collect();
        assert_eq!(colours[0], [UNCUT_R, UNCUT_G, UNCUT_B]);
        for (i, a) in colours.iter().enumerate() {
            for c in a {
                assert!((0.0..=1.0).contains(c), "slot {i}: {a:?}");
            }
            for (j, b) in colours.iter().enumerate().skip(i + 1) {
                assert!(distance(*a, *b) > 0.1, "slots {i} and {j}: {a:?} {b:?}");
            }
        }
        // The hues are 45 degrees apart or more, wood included.
        let wood = hue_deg(colours[0]);
        for (k, c) in colours.iter().enumerate().skip(1) {
            let d = (hue_deg(*c) - wood).rem_euclid(360.0);
            let d = d.min(360.0 - d);
            assert!(d > 44.0, "slot {k} hue is {d} degrees from the wood");
        }
    }

    #[test]
    fn slot_zero_keeps_the_baked_colour_and_an_added_slot_keeps_the_relief() {
        let palette = MaterialPalette::default();
        let deep = [0.5, 0.3, 0.15];
        assert_eq!(palette.shade(MaterialSlot::STOCK, deep), deep);
        let top = palette.shade(MaterialSlot(1), [UNCUT_R, UNCUT_G, UNCUT_B]);
        assert_eq!(top, palette.colour(MaterialSlot(1)));
        let lower = palette.shade(MaterialSlot(1), deep);
        assert!(luma(lower) < luma(top), "{lower:?} vs {top:?}");
    }

    #[test]
    fn a_display_colour_overrides_the_default() {
        let palette =
            MaterialPalette::default().with_colour(MaterialSlot(2), colour_from_rgb8([255, 0, 0]));
        assert_eq!(palette.colour(MaterialSlot(2)), [1.0, 0.0, 0.0]);
        assert_ne!(
            palette.colour(MaterialSlot(1)),
            palette.colour(MaterialSlot(2))
        );
    }

    #[test]
    fn a_mesh_with_no_slot_keeps_its_colours() {
        let mut mesh = StockMesh::empty();
        mesh.vertices = vec![0.0; 6];
        mesh.colors = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let before = mesh.colors.clone();
        apply_material_colours(&mut mesh, &MaterialPalette::default());
        assert_eq!(mesh.colors, before);
    }
}
