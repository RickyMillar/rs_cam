//! The face state slot of the face pipeline (D3, F132).
//!
//! Each STEP model in `face_mesh_data_list` holds one `u32` per face. The
//! fragment stage reads the colour of a face from its word and the palette
//! (`colors::face_palette`). This slot is the ONE writer of those words. On
//! a key change it writes each model's words with one `queue.write_buffer`.
//! A hover or a selection change therefore rewrites `face_count` × 4 B per
//! model and builds no mesh.
//!
//! The key embeds the face mesh key (`meshes.rs`). A mesh rebuild gives new
//! zeroed state buffers, so the key moves with it and the words come back in
//! the same pass. `upload_gpu_data` runs `meshes::upload` first and this
//! slot second.
//!
//! The state word:
//!
//! - bit 0: hovered
//! - bit 1: selected
//! - bits 2-7: the item-state palette slot + 1 (0 = no state)
//! - bits 8-31: reserved (0)

use rs_cam_core::geometry::enriched_mesh::FaceGroupId;
use rs_cam_core::ids::ModelId;

use crate::render::RenderResources;
use crate::render::upload_cache::{self, Refresh, SlotInputs};

use super::Gpu;
use super::meshes::EnrichedInputs;

/// Bit 0 of a state word: the face under the pointer.
const FACE_HOVERED: u32 = 1;
/// Bit 1 of a state word: a selected face.
const FACE_SELECTED: u32 = 1 << 1;
/// The shift of the item-state field (bits 2-7).
const FACE_STATE_SHIFT: u32 = 2;
/// The mask of the item-state field after the shift.
const FACE_STATE_MASK: u32 = 63;

/// The face marks of one pass. Each face is named with its model, so one
/// face index on model A does not mark model B.
#[derive(Debug, Clone, Default)]
pub(super) struct FaceMarks {
    pub(super) hovered: Option<(ModelId, FaceGroupId)>,
    pub(super) selected: Vec<(ModelId, FaceGroupId)>,
    /// The item-state palette slot of a face. Empty until the face path
    /// shows item states (D-Q8: the bits are reserved now).
    pub(super) states: Vec<(ModelId, FaceGroupId, u8)>,
}

/// Everything the face state words are written from.
pub(super) struct FaceStateInputs<'a> {
    /// The face mesh inputs. Their key is part of this key.
    pub(super) mesh: &'a EnrichedInputs<'a>,
    pub(super) marks: FaceMarks,
}

impl SlotInputs for FaceStateInputs<'_> {
    type Key = upload_cache::FaceStateKey;

    fn key(&self) -> Self::Key {
        let Self { mesh, marks } = self;
        let FaceMarks {
            hovered,
            selected,
            states,
        } = marks;
        upload_cache::FaceStateKey {
            mesh: mesh.key(),
            hovered: *hovered,
            selected: selected.clone(),
            states: states.clone(),
        }
    }
}

/// The state words of model `model_id`, one per face. A mark of another
/// model or of a face at or past `face_count` sets no bit.
pub(super) fn face_state_words(model_id: ModelId, face_count: u32, marks: &FaceMarks) -> Vec<u32> {
    let mut words = vec![0u32; face_count as usize];
    let mut mark = |face: FaceGroupId, bits: u32| {
        if let Some(word) = words.get_mut(usize::from(face.0)) {
            *word |= bits;
        }
    };
    for &(model, face, slot) in &marks.states {
        if model == model_id {
            let field = (u32::from(slot) + 1) & FACE_STATE_MASK;
            mark(face, field << FACE_STATE_SHIFT);
        }
    }
    for &(model, face) in &marks.selected {
        if model == model_id {
            mark(face, FACE_SELECTED);
        }
    }
    if let Some((model, face)) = marks.hovered
        && model == model_id
    {
        mark(face, FACE_HOVERED);
    }
    words
}

/// Write the face state words of every drawn STEP model when the key moved.
pub(super) fn upload(inputs: FaceStateInputs<'_>, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    let Refresh::Build(Some(inputs)) =
        upload_cache::refresh(&mut resources.face_state_key, Some(inputs))
    else {
        return;
    };
    for face_mesh in &resources.face_mesh_data_list {
        let words = face_state_words(face_mesh.model_id, face_mesh.face_count, &inputs.marks);
        if !words.is_empty() {
            gpu.queue
                .write_buffer(&face_mesh.face_state, 0, bytemuck::cast_slice(&words));
        }
    }
    resources.upload_stats.face_state_writes += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(raw: usize) -> ModelId {
        ModelId::from_wire(raw)
    }

    #[test]
    fn each_mark_sets_its_own_bits() {
        let a = model(1);
        let marks = FaceMarks {
            hovered: Some((a, FaceGroupId(3))),
            selected: vec![(a, FaceGroupId(1)), (a, FaceGroupId(3))],
            states: vec![(a, FaceGroupId(2), 12), (a, FaceGroupId(3), 14)],
        };
        let words = face_state_words(a, 5, &marks);
        assert_eq!(words.len(), 5);
        assert_eq!(words.first(), Some(&0));
        // Face 1: selected only.
        assert_eq!(words.get(1), Some(&FACE_SELECTED));
        // Face 2: the state field holds slot 12 + 1.
        assert_eq!(words.get(2), Some(&(13 << FACE_STATE_SHIFT)));
        // Face 3: hovered, selected and slot 14 + 1.
        assert_eq!(
            words.get(3),
            Some(&(FACE_HOVERED | FACE_SELECTED | (15 << FACE_STATE_SHIFT)))
        );
        assert_eq!(words.get(4), Some(&0));
        // No mark reaches the reserved bits 8-31.
        assert!(words.iter().all(|w| w >> 8 == 0));
    }

    /// The two-model defect: a bare face index lit the same index on every
    /// STEP model. A mark names its model now.
    #[test]
    fn two_models_do_not_share_a_face_index() {
        let a = model(1);
        let b = model(2);
        let marks = FaceMarks {
            hovered: Some((a, FaceGroupId(3))),
            selected: vec![(a, FaceGroupId(0))],
            states: vec![(a, FaceGroupId(1), 12)],
        };
        let words_a = face_state_words(a, 4, &marks);
        let words_b = face_state_words(b, 4, &marks);
        assert_eq!(words_a.get(3), Some(&FACE_HOVERED));
        assert_eq!(words_a.first(), Some(&FACE_SELECTED));
        assert_eq!(words_b, vec![0; 4]);
    }

    #[test]
    fn a_face_past_the_count_sets_no_bit() {
        let a = model(1);
        let marks = FaceMarks {
            hovered: Some((a, FaceGroupId(9))),
            ..FaceMarks::default()
        };
        assert_eq!(face_state_words(a, 2, &marks), vec![0, 0]);
        assert!(face_state_words(a, 0, &marks).is_empty());
    }
}
