//! The pointer gesture of the viewport (design B, commit B2).
//!
//! `DragState` is a field of `SelectionUi`, so the type that holds the
//! viewport mode also holds its gesture (L-3). The transition is the pure
//! `interaction::input::step`; the frame applies the actions it returns.

/// The pointer gesture in the viewport. One gesture at a time.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum DragState {
    #[default]
    Idle,
    /// The camera turns. `pivot` is the point under the pointer at the drag
    /// start, or `None` (the target). C3a fills it; B2 always writes `None`.
    Orbit {
        pivot: Option<[f32; 3]>,
    },
    Pan,
    /// A rectangle in screen points. `anchor` is the press origin.
    BoxSelect {
        anchor: egui::Pos2,
        to: egui::Pos2,
        op: BoxOp,
    },
    /// The tab editor owns the drag. The content (mark, snapped point) stays
    /// in `TabEditMode::drag`; this is the marker only.
    Tab,
    /// Escape cancelled a box. The button is still down. The release goes
    /// back to `Idle` and sends nothing.
    Cancelled,
}

/// How a box picks: it holds the whole item, or it touches the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxMode {
    Enclose,
    Crossing,
}

/// What a box does to the items it picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxOp {
    Add,
    Remove,
}

impl DragState {
    /// Left to right encloses, right to left crosses (REPORT A.1b SEL-4).
    pub fn box_mode(anchor: egui::Pos2, to: egui::Pos2) -> BoxMode {
        if to.x >= anchor.x {
            BoxMode::Enclose
        } else {
            BoxMode::Crossing
        }
    }
}
