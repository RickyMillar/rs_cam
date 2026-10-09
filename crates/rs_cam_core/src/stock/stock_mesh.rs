//! Triangle mesh data extracted from the stock simulation.
//!
//! STK-14: the toolpath ribbon builders and the per-vertex colour ramps used
//! to live here. They describe a picture, not the stock, so they moved to
//! `export/ribbon.rs`. The crate contract in `../../CLAUDE.md` says the core
//! stays GUI-free; this file holds the mesh container and nothing that
//! chooses a colour.
//!
//! S5 (`planning/stock_additions_2026-10-09/PLAN.md`): the mesh carries the
//! material slot of each vertex ([`StockMesh::material_slots`]). The
//! colour for a slot is chosen in `export/material_colour.rs`.

use crate::stock::material_slot::MaterialSlot;

/// Triangle mesh data exported from stock simulation, suitable for 3D rendering.
#[derive(Clone)]
pub struct StockMesh {
    /// Vertex positions as flat [x, y, z, ...] in f32.
    pub vertices: Vec<f32>,
    /// Triangle indices as flat [i0, i1, ...].
    pub indices: Vec<u32>,
    /// Vertex colors as flat [r, g, b, ...] in f32.
    pub colors: Vec<f32>,
    /// S5: the material slot of each vertex, one byte per vertex. EMPTY when
    /// every vertex is slot 0 (the stock material): a one-material stock
    /// pays no byte. When it is not empty, it has one entry per vertex.
    ///
    /// The mesh builders write the slot of the dexel segment that the vertex
    /// lies on: the top segment for a top surface, the bottom segment for an
    /// underside, the segment itself for a side-grid sheet or a cavity face.
    /// [`Self::slot_at`] reads it.
    pub material_slots: Vec<MaterialSlot>,
}

impl StockMesh {
    /// Create an empty mesh.
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
            material_slots: Vec::new(),
        }
    }

    /// Number of vertices in this mesh.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 3
    }

    /// The material slot of vertex `i`. Slot 0 when the mesh holds no slot
    /// (`material_slots` is empty) or `i` is past its end.
    #[inline]
    #[must_use]
    pub fn slot_at(&self, i: usize) -> MaterialSlot {
        self.material_slots
            .get(i)
            .copied()
            .unwrap_or(MaterialSlot::STOCK)
    }

    /// `true` when some vertex is not slot 0.
    #[must_use]
    pub fn has_added_material(&self) -> bool {
        self.material_slots.iter().any(|s| !s.is_stock())
    }

    /// Transform all vertex positions using a point transform function,
    /// then append the result to `self`.
    pub fn append_transformed<F>(&mut self, other: &StockMesh, transform: F)
    where
        F: Fn(f32, f32, f32) -> (f32, f32, f32),
    {
        let base_vertex = self.vertex_count() as u32;
        let other_vertices = other.vertex_count();

        // M6 (memory programme 2026-10-01): reserve the growth once. A push
        // loop into an empty `Vec` doubles its capacity as it grows, so a mesh
        // of N floats could hold up to 2N floats. `reserve` on an empty `Vec`
        // allocates the exact amount, and it stays amortised when a caller
        // appends many meshes in a loop.
        self.vertices.reserve(other.vertices.len());
        self.indices.reserve(other.indices.len());
        self.colors.reserve(other.colors.len());

        // Transform and append vertices
        let mut i = 0;
        while i + 2 < other.vertices.len() {
            // SAFETY: loop guard ensures i+2 is in bounds
            #[allow(clippy::indexing_slicing)]
            let (x, y, z) = (
                other.vertices[i],
                other.vertices[i + 1],
                other.vertices[i + 2],
            );
            let (tx, ty, tz) = transform(x, y, z);
            self.vertices.push(tx);
            self.vertices.push(ty);
            self.vertices.push(tz);
            i += 3;
        }

        // Offset and append indices
        for idx in &other.indices {
            self.indices.push(idx + base_vertex);
        }

        // Append colors unchanged
        self.colors.extend_from_slice(&other.colors);

        // S5: the slots stay one per vertex. A side with no slots is all
        // slot 0; it gets explicit zeros only when the other side has slots.
        if !other.material_slots.is_empty() || !self.material_slots.is_empty() {
            let base = base_vertex as usize;
            self.material_slots.resize(base, MaterialSlot::STOCK);
            self.material_slots.extend_from_slice(&other.material_slots);
            self.material_slots
                .resize(base + other_vertices, MaterialSlot::STOCK);
        }
    }

    /// Append another mesh (identity transform).
    pub fn append(&mut self, other: &Self) {
        self.append_transformed(other, |x, y, z| (x, y, z));
    }
}

/// Why a display mesh is coarser than the simulation grid.
///
/// A reason of its own, not a `MeasurabilityReason`: a coarse display mesh
/// does not make any metric unmeasurable. The simulation, the collision
/// checks, the cut trace and the column deviations run on the full grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayDegradeReason {
    /// The full-resolution mesh did not fit the memory budget (memory
    /// programme 2026-10-01, degrade 4b).
    MemoryBudget,
}

/// A display mesh built on every `stride`-th row and column of the
/// simulation grid (memory programme 2026-10-01, degrade 4b).
///
/// A simulation result carries `Some` of this only when `stride > 1`.
/// `None` means the display mesh has the full resolution of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayMeshDegrade {
    /// The display stride: the mesh samples every `stride`-th cell on each
    /// axis. Always more than 1.
    pub stride: u32,
    pub reason: DisplayDegradeReason,
}

impl DisplayMeshDegrade {
    /// The degrade record for a requested display stride. A stride of 0 or
    /// 1 is the full resolution and gives `None`. Only the memory budget
    /// asks for a stride above 1, so the reason is
    /// [`DisplayDegradeReason::MemoryBudget`].
    #[must_use]
    pub fn for_stride(stride: u32) -> Option<Self> {
        (stride > 1).then_some(Self {
            stride,
            reason: DisplayDegradeReason::MemoryBudget,
        })
    }

    /// One sentence for the operator, the MCP reply and the diagnostics.
    #[must_use]
    pub fn describe(&self) -> String {
        match self.reason {
            DisplayDegradeReason::MemoryBudget => format!(
                "Display mesh reduced to every {} cell to stay inside the memory budget; the simulation itself runs at full resolution.",
                ordinal(self.stride)
            ),
        }
    }
}

/// `2` → `"2nd"`, `3` → `"3rd"`, `4` → `"4th"`, `11` → `"11th"`.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::{DisplayDegradeReason, DisplayMeshDegrade};

    #[test]
    fn full_resolution_has_no_degrade_record() {
        assert_eq!(DisplayMeshDegrade::for_stride(0), None);
        assert_eq!(DisplayMeshDegrade::for_stride(1), None);
    }

    #[test]
    fn a_stride_names_the_budget_and_the_full_resolution_simulation() {
        let degrade = DisplayMeshDegrade::for_stride(2);
        assert_eq!(
            degrade,
            Some(DisplayMeshDegrade {
                stride: 2,
                reason: DisplayDegradeReason::MemoryBudget
            })
        );
        let text = degrade.map(|d| d.describe()).unwrap_or_default();
        assert_eq!(
            text,
            "Display mesh reduced to every 2nd cell to stay inside the memory budget; the simulation itself runs at full resolution."
        );
        for (stride, word) in [(3, "3rd"), (4, "4th"), (11, "11th"), (21, "21st")] {
            let text = DisplayMeshDegrade::for_stride(stride)
                .map(|d| d.describe())
                .unwrap_or_default();
            assert!(text.contains(&format!("every {word} cell")), "{text}");
        }
    }
}
