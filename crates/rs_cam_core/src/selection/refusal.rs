//! The selection refusal and the selection note (DESIGN 5.1, lead ruling
//! D2/D13).
//!
//! [`super::view`] turns a resolved selection into a refusal or into the
//! items of the op. A refusal names the item, the cause and the fix. A note
//! counts the items that the default rule matched and that the op cannot
//! cut: the op cuts the rest, and the note says what it left out.

use std::fmt;

use crate::compute::catalog::ItemUse;

use super::item::{ItemKind, SourceRef};
use super::key::MatchKey;

/// The name of one item in a refusal or a note: its match key, which holds
/// the source, the kind, the layer, the anchor and the measure. It is the
/// key of the item before the frame door, so it names the item as the
/// drawing holds it (and as a viewport highlight finds it).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRef {
    pub key: MatchKey,
}

impl ItemRef {
    pub fn new(key: MatchKey) -> Self {
        Self { key }
    }
}

/// The noun of a kind, singular and plural.
fn kind_noun(kind: ItemKind, count: usize) -> &'static str {
    let one = count == 1;
    match kind {
        ItemKind::Loop if one => "closed loop",
        ItemKind::Loop => "closed loops",
        ItemKind::Path if one => "open path",
        ItemKind::Path => "open paths",
        ItemKind::Point if one => "point",
        ItemKind::Point => "points",
        ItemKind::Circle if one => "circle",
        ItemKind::Circle => "circles",
        ItemKind::Face if one => "face",
        ItemKind::Face => "faces",
    }
}

/// The text " on layer 'L'", or nothing for an item with no layer.
fn layer_phrase(layer: Option<&str>) -> String {
    layer.map_or_else(String::new, |l| format!(" on layer '{l}'"))
}

/// What an op of `use_` cuts, as a phrase after the op name.
fn use_phrase(use_: ItemUse) -> &'static str {
    match use_ {
        ItemUse::None => "reads no drawing item",
        ItemUse::Regions => "cuts closed areas only (closed loops, circles, faces)",
        ItemUse::Curves => "follows lines only (closed loops, circles, open paths)",
        ItemUse::Holes { .. } => "drills points and circles only",
    }
}

impl fmt::Display for ItemRef {
    /// "open path on layer 'TEXT' (starts at X 12.000 Y 40.500, 86.200 mm
    /// long)".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let k = &self.key;
        let [x, y] = k.anchor;
        let m = k.measure;
        let at = match k.kind {
            ItemKind::Loop => format!("centre X {x:.3} Y {y:.3}, {m:.3} mm across"),
            ItemKind::Path => format!("starts at X {x:.3} Y {y:.3}, {m:.3} mm long"),
            ItemKind::Point => format!("at X {x:.3} Y {y:.3}"),
            ItemKind::Circle => format!("centre X {x:.3} Y {y:.3}, diameter {m:.3} mm"),
            ItemKind::Face => format!("centre X {x:.3} Y {y:.3}, {m:.3} mm across"),
        };
        write!(
            f,
            "{}{} ({at})",
            kind_noun(k.kind, 1),
            layer_phrase(k.layer.as_deref())
        )
    }
}

/// The items of one kind and one layer that the default rule matched and
/// that the op cannot cut. The op leaves them out and cuts the rest (lead
/// ruling D2/D13).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionNote {
    pub kind: ItemKind,
    pub layer: Option<String>,
    /// How many items of this kind and layer the op left out. Never 0.
    pub count: usize,
    /// The first item left out, in import order.
    pub first: ItemRef,
    /// How the op uses its items.
    pub item_use: ItemUse,
}

impl fmt::Display for SelectionNote {
    /// "2 open paths on layer 'TEXT' are not cut: the op cuts closed areas
    /// only (...). The first: open path (...)." An open path also gets the
    /// fix that suits the op ([`note_fix`]).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = if self.count == 1 { "is" } else { "are" };
        write!(
            f,
            "{} {}{} {verb} not cut: the op {}. The first: {}.",
            self.count,
            kind_noun(self.kind, self.count),
            layer_phrase(self.layer.as_deref()),
            use_phrase(self.item_use),
            self.first
        )?;
        if let Some(fix) = note_fix(self.kind, self.item_use) {
            write!(f, " {fix}")?;
        }
        Ok(())
    }
}

/// The fix a note gives for a skipped kind, for an op of `item_use`. Only
/// an open path gets one: it is a likely drawing defect (lead ruling
/// "notes", P2.2). The fix depends on the op: a closed loop serves an area
/// op, but a drill drills no loop, so "close it" is no fix for a drill.
/// The match has no wildcard, so a new kind or a new use states its answer.
fn note_fix(kind: ItemKind, item_use: ItemUse) -> Option<&'static str> {
    match (kind, item_use) {
        (ItemKind::Path, ItemUse::Regions) => {
            Some("Use Trace or Project Curve for an open path, or close it in the drawing.")
        }
        (ItemKind::Path, ItemUse::Holes { .. }) => {
            Some("To cut the open path, add a Trace or Project Curve operation.")
        }
        // An op that follows lines cuts an open path, and an op that reads
        // no item skips nothing: neither gives a path note.
        (ItemKind::Path, ItemUse::Curves | ItemUse::None) => None,
        (
            ItemKind::Loop | ItemKind::Point | ItemKind::Circle | ItemKind::Face,
            ItemUse::Regions | ItemUse::Curves | ItemUse::Holes { .. } | ItemUse::None,
        ) => None,
    }
}

/// Why a selection refuses (DESIGN 5.1).
#[derive(Debug, Clone, PartialEq)]
pub enum RefusalCause {
    /// The op has nothing to cut. `skipped` holds the items the default
    /// rule matched and the op cannot cut (empty when the rules matched
    /// nothing at all).
    Empty { skipped: Vec<SelectionNote> },
    /// No item matches a stored pick (the drawing changed).
    LostPick,
    /// Two or more items match one stored pick (a duplicate entity).
    AmbiguousPick { matches: usize },
    /// The user named an item (a pick, or a rule that names its kind) that
    /// the op cannot cut: an open path in a closed-area op, and so on.
    KindNotCut { kind: ItemKind, item_use: ItemUse },
    /// A picked face that is not a flat horizontal face (P2.4).
    FaceNotFlat,
    /// A rule or a pick names a model that does not exist. No fallback to
    /// another model.
    MissingSource(SourceRef),
    /// A rule names a layer that no item of its source has (S21).
    UnknownLayer(String),
    /// A selection edit names a rule index that does not exist.
    NoSuchRule { index: usize, rules: usize },
}

/// One structured refusal: the item, the cause and the fix.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionRefusal {
    /// The op label. `view` does not know the op; the caller sets it with
    /// [`SelectionRefusal::for_op`].
    pub op: Option<&'static str>,
    /// The item the refusal names, when one item is the cause.
    pub item: Option<ItemRef>,
    pub cause: RefusalCause,
}

impl SelectionRefusal {
    pub fn new(item: Option<ItemRef>, cause: RefusalCause) -> Self {
        Self {
            op: None,
            item,
            cause,
        }
    }

    /// The same refusal, with the op label for the text.
    #[must_use]
    pub fn for_op(mut self, op: &'static str) -> Self {
        self.op = Some(op);
        self
    }

    /// The fix: one text per cause.
    pub fn fix(&self) -> &'static str {
        match self.cause {
            RefusalCause::Empty { .. } => {
                "Add items that the operation can cut to its selection, or use \
                 an operation that cuts the items of the drawing"
            }
            RefusalCause::LostPick => "Pick the item again, or remove the pick from the selection",
            RefusalCause::AmbiguousPick { .. } => {
                "Remove the duplicate entity from the drawing, or remove the \
                 pick and select the item by a rule"
            }
            RefusalCause::KindNotCut { .. } => {
                "Remove the item from the selection, or use an operation that \
                 cuts this kind of item (Trace or Project Curve for an open \
                 path, Drill for a point)"
            }
            RefusalCause::FaceNotFlat => {
                "Pick a flat horizontal face, or remove the face from the selection"
            }
            RefusalCause::MissingSource(_) => {
                "Import the model again, or remove the rule or the pick that \
                 names it"
            }
            RefusalCause::UnknownLayer(_) => {
                "Correct the layer name in the rule, or add the layer to the drawing"
            }
            RefusalCause::NoSuchRule { .. } => "Name a rule that the selection holds",
        }
    }

    /// The cause as a phrase.
    fn cause_text(&self) -> String {
        match &self.cause {
            RefusalCause::Empty { skipped } if skipped.is_empty() => {
                "the selection holds no item to cut".to_owned()
            }
            RefusalCause::Empty { skipped } => {
                let notes: Vec<String> = skipped.iter().map(ToString::to_string).collect();
                format!("nothing is left to cut. {}", notes.join(" "))
            }
            RefusalCause::LostPick => "no item of the drawing matches this pick".to_owned(),
            RefusalCause::AmbiguousPick { matches } => {
                format!("{matches} items of the drawing match this pick")
            }
            RefusalCause::KindNotCut { kind, item_use } => format!(
                "the selection names a {}, and the op {}",
                kind_noun(*kind, 1),
                use_phrase(*item_use)
            ),
            RefusalCause::FaceNotFlat => "the face is not flat and horizontal".to_owned(),
            RefusalCause::MissingSource(source) => {
                let name = match source {
                    SourceRef::ToolpathModel => "the model of the toolpath".to_owned(),
                    SourceRef::Model(id) => format!("model {}", id.0),
                    SourceRef::Stock => "the stock".to_owned(),
                };
                format!("the selection reads {name}, which does not exist")
            }
            RefusalCause::UnknownLayer(layer) => {
                format!("a rule names layer '{layer}', which holds no item")
            }
            RefusalCause::NoSuchRule { index, rules } => {
                format!("rule {index} does not exist (the selection holds {rules})")
            }
        }
    }
}

impl fmt::Display for SelectionRefusal {
    /// One sentence: the op and the item, the cause, the fix.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(op) = self.op {
            write!(f, "{op}: ")?;
        }
        if let Some(item) = &self.item {
            write!(f, "{item}: ")?;
        }
        write!(f, "{}. Fix: {}.", self.cause_text(), self.fix())
    }
}

impl std::error::Error for SelectionRefusal {}
