//! The edit ledger of a drawing: join and close acts on its open paths
//! (geometry selection P4.9, lead ruling L-11).
//!
//! An import never closes a gap with a chord in silence (P1). When the
//! user wants a gap closed, the user says so: each act is one
//! [`DrawingEdit`], and the model keeps the list of them (the ledger). The
//! project file stores the ledger. The session replays it in order on the
//! items of the import door after every import, reload, rescale and relink
//! ([`apply_drawing_edits`]), so a re-import keeps the edits.
//!
//! - The ledger is in FILE units: the frame of the P4.8 join tolerance,
//!   the units of the file as the door reads it (a DXF after `$INSUNITS`,
//!   an SVG in mm after its declared size and every transform, O-2),
//!   before the declared units (the user scale). Each key, length and
//!   tolerance of a stored edit is in that frame, so a rescale changes no
//!   stored edit. [`DrawingEdit::scaled`] converts: the session door
//!   converts a new edit from model mm once, and the replay converts the
//!   stored edits to model mm. The panel and MCP show model mm.
//! - An edit names a path by its [`MatchKey`] and matches it by geometry
//!   only ([`MatchKey::names_same_geometry`]): the ledger belongs to one
//!   model, so the source is not part of the match.
//! - A new item comes from [`Item::new`] on the edited shape, as a chained
//!   DXF path does. An unchanged re-import replays to the same keys, so a
//!   pick on a joined path survives a reload. A pick on a piece that a join
//!   consumed flags as lost.
//! - A new item takes the place of its lowest piece and the layer of its
//!   first piece (the DXF chain rule).
//! - An edit that does not apply gives an [`EditLost`]. The replay keeps
//!   the edit in the ledger and reports it. It is never dropped in silence.
//! - [`DrawingEdit::JoinWithin`] calls the one join kernel
//!   ([`super::join::join_open_pieces`]). No second join code exists.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::geo::P2;
use crate::polygon::Polygon2;

use super::item::{Item, ItemKind, ItemSet, Shape, SourceRef};
use super::join::{CLOSE_TOL_MM, OpenPiece, join_open_pieces, polygon_from_unknown_closure};
use super::key::{KEY_MATCH_EPS_MM, MatchKey};

/// One act of the user on the open paths of a drawing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingEdit {
    /// Chain the open paths whose ends meet within `tolerance_mm` (mm of
    /// the model). `layer` of `None` joins on every layer; a chain never
    /// crosses a layer (DESIGN D14).
    JoinWithin {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layer: Option<String>,
        tolerance_mm: f64,
    },
    /// Close one open path with a straight line from its end to its start.
    CloseWithLine { path: MatchKey },
    /// Close one open path: its end point moves onto its start point. No
    /// new segment is added; the last segment ends at the start.
    MoveEndsTogether { path: MatchKey },
    /// Join two open paths of one layer with a line between their nearest
    /// ends.
    JoinTwo { a: MatchKey, b: MatchKey },
}

impl DrawingEdit {
    /// The same edit with each key in the form the ledger stores: the
    /// source is [`SourceRef::ToolpathModel`] ("this model"). The ledger
    /// belongs to one model, so a key from the GUI (bound to
    /// `Model(id)`) and a key from MCP give one stored edit.
    #[must_use]
    pub fn in_ledger_form(self) -> Self {
        let own = |key: MatchKey| MatchKey {
            source: SourceRef::ToolpathModel,
            ..key
        };
        match self {
            DrawingEdit::JoinWithin { .. } => self,
            DrawingEdit::CloseWithLine { path } => DrawingEdit::CloseWithLine { path: own(path) },
            DrawingEdit::MoveEndsTogether { path } => {
                DrawingEdit::MoveEndsTogether { path: own(path) }
            }
            DrawingEdit::JoinTwo { a, b } => DrawingEdit::JoinTwo {
                a: own(a),
                b: own(b),
            },
        }
    }

    /// The same edit with each key anchor, key measure and tolerance
    /// multiplied by `factor`. File units to model mm: the declared scale
    /// (`ModelUnits::scale_factor`); model mm to file units: its inverse.
    /// A factor of 1 gives the edit unchanged.
    #[must_use]
    pub fn scaled(self, factor: f64) -> Self {
        if (factor - 1.0).abs() < 1e-12 {
            return self;
        }
        let key = |key: MatchKey| MatchKey {
            anchor: [key.anchor[0] * factor, key.anchor[1] * factor],
            measure: key.measure * factor,
            ..key
        };
        match self {
            DrawingEdit::JoinWithin {
                layer,
                tolerance_mm,
            } => DrawingEdit::JoinWithin {
                layer,
                tolerance_mm: tolerance_mm * factor,
            },
            DrawingEdit::CloseWithLine { path } => DrawingEdit::CloseWithLine { path: key(path) },
            DrawingEdit::MoveEndsTogether { path } => {
                DrawingEdit::MoveEndsTogether { path: key(path) }
            }
            DrawingEdit::JoinTwo { a, b } => DrawingEdit::JoinTwo {
                a: key(a),
                b: key(b),
            },
        }
    }

    /// The edit as one line, for the ledger list of the model panel and
    /// MCP `inspect_model`. Call it on the edit in model mm
    /// ([`Self::scaled`] by the declared scale).
    pub fn label(&self) -> String {
        match self {
            DrawingEdit::JoinWithin {
                layer: Some(layer),
                tolerance_mm,
            } => format!("Join the open paths on layer {layer} within {tolerance_mm} mm"),
            DrawingEdit::JoinWithin {
                layer: None,
                tolerance_mm,
            } => format!("Join the open paths within {tolerance_mm} mm"),
            DrawingEdit::CloseWithLine { path } => {
                format!("Close {} with a line", path_name(path))
            }
            DrawingEdit::MoveEndsTogether { path } => {
                format!("Move the ends of {} together", path_name(path))
            }
            DrawingEdit::JoinTwo { a, b } => {
                format!("Join {} and {}", path_name(a), path_name(b))
            }
        }
    }
}

/// "the path on layer TEXT at (12.00, 4.50), 86.20 mm": the words for one
/// path key.
fn path_name(key: &MatchKey) -> String {
    let layer = key
        .layer
        .as_deref()
        .map_or_else(String::new, |l| format!(" on layer {l}"));
    format!(
        "the path{layer} at ({:.2}, {:.2}), {:.2} mm",
        key.anchor[0], key.anchor[1], key.measure
    )
}

/// What one edit did.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EditEffect {
    /// The open paths the edit consumed.
    pub pieces: usize,
    /// The items the edit made.
    pub shapes: usize,
    /// The made items that are closed loops.
    pub closed: usize,
    /// The largest gap the edit closed or bridged, mm: the line of a close
    /// or of a two-path join, the move of a "move ends together". 0 for a
    /// join within a tolerance.
    pub gap_mm: f64,
}

/// Why an edit does not apply to the items. The replay keeps the edit and
/// reports this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "cause")]
pub enum EditLost {
    /// No item has the key: the path is gone (a changed file, or an earlier
    /// edit consumed it).
    NoItem { key: MatchKey },
    /// Two or more items have the key.
    Ambiguous { key: MatchKey, count: usize },
    /// The key names an item that is not an open path.
    NotAPath { key: MatchKey },
    /// The close gives a loop of fewer than 3 points.
    TooFewPoints { key: MatchKey },
    /// The two paths of a join are on different layers (DESIGN D14).
    AcrossLayers {
        a: Option<String>,
        b: Option<String>,
    },
    /// The two keys of a join name one path.
    SamePath { key: MatchKey },
    /// No item is on the layer of a join.
    NoLayer { layer: String },
    /// The tolerance is not a positive number.
    BadTolerance { tolerance_mm: f64 },
    /// The edited shape breaks an item invariant.
    BadShape { reason: String },
}

impl EditLost {
    /// The reason as a sentence, for the model panel and MCP.
    pub fn label(&self) -> String {
        match self {
            EditLost::NoItem { key } => format!("Lost: no item is {}", path_name(key)),
            EditLost::Ambiguous { key, count } => {
                format!("Lost: {count} items are {}", path_name(key))
            }
            EditLost::NotAPath { key } => {
                format!(
                    "The item at ({:.2}, {:.2}) is not an open path",
                    key.anchor[0], key.anchor[1]
                )
            }
            EditLost::TooFewPoints { key } => format!(
                "{} has too few points to close",
                capitalise(&path_name(key))
            ),
            EditLost::AcrossLayers { a, b } => format!(
                "A join cannot cross layers ({} and {})",
                a.as_deref().unwrap_or("no layer"),
                b.as_deref().unwrap_or("no layer")
            ),
            EditLost::SamePath { key } => {
                format!("Both ends name {}", path_name(key))
            }
            EditLost::NoLayer { layer } => format!("Lost: no item is on layer {layer}"),
            EditLost::BadTolerance { tolerance_mm } => {
                format!("The tolerance must be a positive number of mm, not {tolerance_mm}")
            }
            EditLost::BadShape { reason } => format!("The edit gives a bad shape: {reason}"),
        }
    }
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The outcome of one edit of a replay: the edit's position in the ledger
/// and what it did, or why it does not apply.
#[derive(Debug, Clone, PartialEq)]
pub struct EditOutcome {
    pub edit: usize,
    pub result: Result<EditEffect, EditLost>,
}

/// Apply `edits` in order to `items`. Each edit sees the items after the
/// edits before it. An edit that does not apply changes nothing and gives
/// an `Err` outcome; the replay goes on with the next edit.
///
/// An empty ledger gives a set with the same items, in the same order.
pub fn apply_drawing_edits(items: &ItemSet, edits: &[DrawingEdit]) -> (ItemSet, Vec<EditOutcome>) {
    let mut list: Vec<Item> = items.iter().cloned().collect();
    let mut outcomes = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        let result = apply_one(&list, edit);
        let result = match result {
            Ok((next, effect)) => {
                list = next;
                Ok(effect)
            }
            Err(lost) => Err(lost),
        };
        outcomes.push(EditOutcome {
            edit: index,
            result,
        });
    }
    (ItemSet::new(list), outcomes)
}

/// One edit on the item list: the new list and the effect.
fn apply_one(items: &[Item], edit: &DrawingEdit) -> Result<(Vec<Item>, EditEffect), EditLost> {
    match edit {
        DrawingEdit::JoinWithin {
            layer,
            tolerance_mm,
        } => join_within(items, layer.as_deref(), *tolerance_mm),
        DrawingEdit::CloseWithLine { path } => {
            let (at, item) = find_path(items, path)?;
            let mut pts = path_points(item);
            // An end on the start gives no line: drop the repeat.
            if let (Some(first), Some(last)) = (pts.first(), pts.last())
                && pts.len() >= 2
                && (last - first).norm() < KEY_MATCH_EPS_MM
            {
                pts.pop();
            }
            let gap = end_gap(&pts);
            let item = close_points(item, pts, path)?;
            Ok((replace(items, at, None, item), closed_effect(1, gap)))
        }
        DrawingEdit::MoveEndsTogether { path } => {
            let (at, item) = find_path(items, path)?;
            let mut pts = path_points(item);
            let gap = end_gap(&pts);
            // The end point moves onto the start: the ring closes from the
            // point before the end to the start.
            pts.pop();
            let item = close_points(item, pts, path)?;
            Ok((replace(items, at, None, item), closed_effect(1, gap)))
        }
        DrawingEdit::JoinTwo { a, b } => {
            let (ia, first) = find_path(items, a)?;
            let (ib, second) = find_path(items, b)?;
            if ia == ib {
                return Err(EditLost::SamePath { key: a.clone() });
            }
            if first.layer() != second.layer() {
                return Err(EditLost::AcrossLayers {
                    a: first.layer().map(str::to_owned),
                    b: second.layer().map(str::to_owned),
                });
            }
            let (pts, gap) = join_two_points(&path_points(first), &path_points(second));
            let polygon = polygon_from_unknown_closure(pts, CLOSE_TOL_MM);
            let closed = polygon.closed;
            let item = new_item(first, polygon)?;
            let (keep, drop) = (ia.min(ib), ia.max(ib));
            Ok((
                replace(items, keep, Some(drop), item),
                EditEffect {
                    pieces: 2,
                    shapes: 1,
                    closed: usize::from(closed),
                    gap_mm: gap,
                },
            ))
        }
    }
}

/// [`DrawingEdit::JoinWithin`]: the open paths of `layer` (or of every
/// layer) go through the one join kernel. A chain of two or more paths, or
/// one path whose ends meet within the tolerance, becomes one new item at
/// the place of its lowest piece.
fn join_within(
    items: &[Item],
    layer: Option<&str>,
    tolerance_mm: f64,
) -> Result<(Vec<Item>, EditEffect), EditLost> {
    if !(tolerance_mm.is_finite() && tolerance_mm > 0.0) {
        return Err(EditLost::BadTolerance { tolerance_mm });
    }
    if let Some(name) = layer
        && !items.iter().any(|i| i.layer() == Some(name))
    {
        return Err(EditLost::NoLayer {
            layer: name.to_owned(),
        });
    }
    // The pieces arrive in index order, as the kernel asks.
    let pieces: Vec<OpenPiece> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == ItemKind::Path)
        .filter(|(_, item)| layer.is_none_or(|name| item.layer() == Some(name)))
        .map(|(index, item)| OpenPiece {
            index,
            layer: item.layer().map(Arc::from),
            pts: path_points(item),
        })
        .collect();
    let piece_at: std::collections::HashSet<usize> = pieces.iter().map(|p| p.index).collect();
    let (shapes, _) = join_open_pieces(&pieces, tolerance_mm);
    let mut effect = EditEffect::default();
    let mut made: std::collections::HashMap<usize, Item> = std::collections::HashMap::new();
    let mut unchanged: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for shape in shapes {
        if shape.pieces < 2 && !shape.polygon.closed {
            // One path that stays open: the item does not change.
            unchanged.insert(shape.index);
            continue;
        }
        let Some(first) = items.get(shape.index) else {
            continue;
        };
        effect.pieces += shape.pieces;
        effect.shapes += 1;
        effect.closed += usize::from(shape.polygon.closed);
        made.insert(shape.index, new_item(first, shape.polygon)?);
    }
    // Every piece is in one shape, and a shape sits at its lowest piece. A
    // piece that is not the place of a shape was consumed by a join.
    let out = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match made.remove(&index) {
            Some(new) => Some(new),
            None if piece_at.contains(&index) && !unchanged.contains(&index) => None,
            None => Some(item.clone()),
        })
        .collect();
    Ok((out, effect))
}

/// The points of an open path item (`Item::new` keeps a path at 2 or more
/// points).
fn path_points(item: &Item) -> Vec<P2> {
    match item.shape() {
        Shape::Path(p) => p.exterior.clone(),
        _ => Vec::new(),
    }
}

/// The distance between the two ends of a point list.
fn end_gap(pts: &[P2]) -> f64 {
    match (pts.first(), pts.last()) {
        (Some(a), Some(b)) => (b - a).norm(),
        _ => 0.0,
    }
}

fn closed_effect(pieces: usize, gap_mm: f64) -> EditEffect {
    EditEffect {
        pieces,
        shapes: 1,
        closed: 1,
        gap_mm,
    }
}

/// The one open path that `key` names, with its position.
fn find_path<'a>(items: &'a [Item], key: &MatchKey) -> Result<(usize, &'a Item), EditLost> {
    if key.kind != ItemKind::Path {
        return Err(EditLost::NotAPath { key: key.clone() });
    }
    let hits: Vec<(usize, &Item)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.key().names_same_geometry(key))
        .collect();
    match hits.as_slice() {
        [] => Err(EditLost::NoItem { key: key.clone() }),
        [one] => Ok(*one),
        many => Err(EditLost::Ambiguous {
            key: key.clone(),
            count: many.len(),
        }),
    }
}

/// A closed loop of `pts` with the source and layer of `from`.
fn close_points(from: &Item, pts: Vec<P2>, key: &MatchKey) -> Result<Item, EditLost> {
    if pts.len() < 3 {
        return Err(EditLost::TooFewPoints { key: key.clone() });
    }
    let mut ring = Polygon2::new(pts);
    ring.ensure_winding();
    new_item(from, ring)
}

/// The item of `polygon` (a loop when closed, else a path) with the source
/// and layer of `first`, its first piece. [`Item::new`] computes the key.
fn new_item(first: &Item, polygon: Polygon2) -> Result<Item, EditLost> {
    let shape = if polygon.closed {
        Shape::Loop(polygon)
    } else {
        Shape::Path(polygon)
    };
    Item::new(first.source(), first.layer().map(Arc::from), shape).map_err(|e| EditLost::BadShape {
        reason: e.to_string(),
    })
}

/// The list with `item` at `at` and the item at `drop` removed.
fn replace(items: &[Item], at: usize, drop: Option<usize>, item: Item) -> Vec<Item> {
    let mut item = Some(item);
    items
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != drop)
        .map(|(index, old)| {
            if index == at {
                item.take().unwrap_or_else(|| old.clone())
            } else {
                old.clone()
            }
        })
        .collect()
}

/// The points of two paths joined at their nearest ends, and the gap the
/// line bridges. `a` keeps its direction where it can. At equal distances
/// the order of the four cases decides.
fn join_two_points(a: &[P2], b: &[P2]) -> (Vec<P2>, f64) {
    let (Some(&a0), Some(&a1), Some(&b0), Some(&b1)) = (a.first(), a.last(), b.first(), b.last())
    else {
        return (a.to_vec(), 0.0);
    };
    let rev = |pts: &[P2]| pts.iter().rev().copied().collect::<Vec<P2>>();
    // (gap, head, tail): the path is head then tail.
    let cases: [(f64, Vec<P2>, Vec<P2>); 4] = [
        ((b0 - a1).norm(), a.to_vec(), b.to_vec()),
        ((b1 - a1).norm(), a.to_vec(), rev(b)),
        ((a0 - b1).norm(), b.to_vec(), a.to_vec()),
        ((b0 - a0).norm(), rev(a), b.to_vec()),
    ];
    let mut best = 0;
    for (index, case) in cases.iter().enumerate() {
        if let Some(current) = cases.get(best)
            && case.0 < current.0
        {
            best = index;
        }
    }
    let [c0, c1, c2, c3] = cases;
    let (gap, mut head, tail) = match best {
        0 => c0,
        1 => c1,
        2 => c2,
        _ => c3,
    };
    let mut tail = tail.into_iter();
    if gap < KEY_MATCH_EPS_MM {
        // The ends meet: the joint point is not repeated.
        tail.next();
    }
    head.extend(tail);
    (head, gap)
}

/// One open path of a drawing, as the model panel and MCP `inspect_model`
/// list it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenShape {
    /// The position in the item set.
    pub index: usize,
    pub key: MatchKey,
    pub layer: Option<String>,
    /// The length of the path, mm.
    pub length_mm: f64,
    /// The distance from its end to its start, mm: the line a close adds.
    pub gap_mm: f64,
    /// True when the path has 3 or more points, so a close with a line
    /// gives a loop.
    pub can_close_with_line: bool,
    /// True when the path has 4 or more points, so a move of the end onto
    /// the start gives a loop.
    pub can_move_ends_together: bool,
}

/// The open paths of `items`, in item order.
pub fn open_shapes(items: &ItemSet) -> Vec<OpenShape> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item.shape() {
            Shape::Path(p) => {
                let n = p.exterior.len();
                let repeat = n >= 2 && end_gap(&p.exterior) < KEY_MATCH_EPS_MM;
                let points = if repeat { n - 1 } else { n };
                Some(OpenShape {
                    index,
                    key: item.key().clone(),
                    layer: item.layer().map(str::to_owned),
                    length_mm: item.key().measure,
                    gap_mm: end_gap(&p.exterior),
                    can_close_with_line: points >= 3,
                    can_move_ends_together: n >= 4,
                })
            }
            _ => None,
        })
        .collect()
}

/// The ledger of one model: the edits the project file stores, the items
/// of the import door they replay on, and the outcome of the last replay.
///
/// `Default` is the empty ledger: the model's items are the door items.
#[derive(Debug, Clone, Default)]
pub struct DrawingLedger {
    edits: Vec<DrawingEdit>,
    /// The items of the door before the edits; `None` while the ledger is
    /// empty (the model's items ARE the door items, the same `Arc`).
    door_items: Option<Arc<ItemSet>>,
    outcomes: Vec<EditOutcome>,
}

impl DrawingLedger {
    /// A ledger of `edits` with no replay yet (a model that did not load).
    pub fn unreplayed(edits: Vec<DrawingEdit>) -> Self {
        Self {
            edits,
            door_items: None,
            outcomes: Vec::new(),
        }
    }

    /// The stored edits, in order.
    pub fn edits(&self) -> &[DrawingEdit] {
        &self.edits
    }

    /// The outcome of each edit at the last replay, in ledger order.
    pub fn outcomes(&self) -> &[EditOutcome] {
        &self.outcomes
    }

    /// The count of edits that do not apply.
    pub fn lost_count(&self) -> usize {
        self.outcomes.iter().filter(|o| o.result.is_err()).count()
    }

    /// The items of the import door: the stored door items, else `items`
    /// (the model's items, when the ledger is empty).
    pub fn door_items(&self, items: &Arc<ItemSet>) -> Arc<ItemSet> {
        self.door_items
            .as_ref()
            .map_or_else(|| Arc::clone(items), Arc::clone)
    }

    /// The stored edits in model mm: each edit [`DrawingEdit::scaled`] by
    /// `scale`, the declared scale of the model.
    pub fn edits_in_model_mm(&self, scale: f64) -> Vec<DrawingEdit> {
        self.edits
            .iter()
            .cloned()
            .map(|edit| edit.scaled(scale))
            .collect()
    }

    /// Replay `edits` (file units) on `door` (the door items in model mm,
    /// after the declared scale `scale`) and give the edited items. The
    /// edits apply in model mm ([`Self::edits_in_model_mm`]); the stored
    /// edits stay in file units. An empty ledger gives `door` itself (the
    /// same `Arc`): nothing changes.
    pub fn replay(
        &mut self,
        door: Arc<ItemSet>,
        edits: Vec<DrawingEdit>,
        scale: f64,
    ) -> Arc<ItemSet> {
        self.edits = edits;
        if self.edits.is_empty() {
            self.door_items = None;
            self.outcomes.clear();
            return door;
        }
        let (edited, outcomes) = apply_drawing_edits(&door, &self.edits_in_model_mm(scale));
        self.door_items = Some(door);
        self.outcomes = outcomes;
        Arc::new(edited)
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    fn path(layer: &str, pts: &[(f64, f64)]) -> Item {
        Item::new(
            SourceRef::ToolpathModel,
            Some(Arc::from(layer)),
            Shape::Path(Polygon2::open_path(
                pts.iter().map(|&(x, y)| P2::new(x, y)).collect(),
            )),
        )
        .unwrap()
    }

    fn square_loop(layer: &str) -> Item {
        Item::new(
            SourceRef::ToolpathModel,
            Some(Arc::from(layer)),
            Shape::Loop(Polygon2::new(vec![
                P2::new(50.0, 50.0),
                P2::new(60.0, 50.0),
                P2::new(60.0, 60.0),
                P2::new(50.0, 60.0),
            ])),
        )
        .unwrap()
    }

    /// A U on layer A with a 0.3 gap at the top, two pieces of an L on A
    /// 0.2 apart, a loop, and a piece on layer B.
    fn fixture() -> ItemSet {
        ItemSet::new(vec![
            path(
                "A",
                &[
                    (0.0, 10.0),
                    (0.0, 0.0),
                    (10.0, 0.0),
                    (10.0, 10.0),
                    (0.3, 10.0),
                ],
            ),
            path("A", &[(20.0, 0.0), (30.0, 0.0)]),
            square_loop("A"),
            path("A", &[(30.2, 0.0), (30.2, 10.0)]),
            path("B", &[(30.4, 10.0), (40.0, 10.0)]),
        ])
    }

    fn kinds(set: &ItemSet) -> Vec<ItemKind> {
        set.iter().map(Item::kind).collect()
    }

    #[test]
    fn an_empty_ledger_changes_nothing_p4_9() {
        let set = fixture();
        let (out, outcomes) = apply_drawing_edits(&set, &[]);
        assert!(outcomes.is_empty());
        let keys = |s: &ItemSet| s.iter().map(|i| i.key().clone()).collect::<Vec<_>>();
        assert_eq!(keys(&out), keys(&set));
    }

    #[test]
    fn join_within_uses_the_kernel_and_keeps_layers_p4_9() {
        let set = fixture();
        let (out, outcomes) = apply_drawing_edits(
            &set,
            &[DrawingEdit::JoinWithin {
                layer: None,
                tolerance_mm: 0.5,
            }],
        );
        // The U closes (gap 0.3 < 0.5); the L pieces join at index 1; the
        // B piece does not join the A piece (D14).
        assert_eq!(
            kinds(&out),
            vec![
                ItemKind::Loop,
                ItemKind::Path,
                ItemKind::Loop,
                ItemKind::Path
            ]
        );
        let effect = outcomes[0].result.as_ref().unwrap();
        assert_eq!((effect.pieces, effect.shapes, effect.closed), (3, 2, 1));
        assert_eq!(out.iter().nth(1).unwrap().layer(), Some("A"));
        assert_eq!(out.iter().nth(3).unwrap().layer(), Some("B"));
    }

    #[test]
    fn close_move_and_join_two_p4_9() {
        let set = fixture();
        let u = set.iter().next().unwrap().key().clone();
        let (closed, outcomes) =
            apply_drawing_edits(&set, &[DrawingEdit::CloseWithLine { path: u.clone() }]);
        assert_eq!(closed.iter().next().unwrap().kind(), ItemKind::Loop);
        assert!((outcomes[0].result.as_ref().unwrap().gap_mm - 0.3).abs() < 1e-9);
        let (moved, _) = apply_drawing_edits(&set, &[DrawingEdit::MoveEndsTogether { path: u }]);
        let Shape::Loop(ring) = moved.iter().next().unwrap().shape() else {
            panic!("a loop")
        };
        assert_eq!(ring.exterior.len(), 4);

        let a = set.iter().nth(1).unwrap().key().clone();
        let b = set.iter().nth(3).unwrap().key().clone();
        let (joined, outcomes) = apply_drawing_edits(&set, &[DrawingEdit::JoinTwo { a, b }]);
        assert_eq!(joined.len(), 4);
        assert!((outcomes[0].result.as_ref().unwrap().gap_mm - 0.2).abs() < 1e-9);
        let Shape::Path(p) = joined.iter().nth(1).unwrap().shape() else {
            panic!("a path")
        };
        assert_eq!(p.exterior.len(), 4);
    }

    #[test]
    fn a_cross_layer_join_and_a_lost_key_report_p4_9() {
        let set = fixture();
        let a = set.iter().nth(3).unwrap().key().clone();
        let b = set.iter().nth(4).unwrap().key().clone();
        let (_, outcomes) = apply_drawing_edits(&set, &[DrawingEdit::JoinTwo { a: a.clone(), b }]);
        assert!(matches!(
            outcomes[0].result,
            Err(EditLost::AcrossLayers { .. })
        ));
        // A join consumes the piece; a close of it later is lost.
        let (_, outcomes) = apply_drawing_edits(
            &set,
            &[
                DrawingEdit::JoinWithin {
                    layer: Some("A".into()),
                    tolerance_mm: 0.5,
                },
                DrawingEdit::CloseWithLine { path: a },
            ],
        );
        assert!(outcomes[0].result.is_ok());
        assert!(matches!(outcomes[1].result, Err(EditLost::NoItem { .. })));
    }

    #[test]
    fn the_edit_json_is_stable_p4_9() {
        let set = fixture();
        let edit = DrawingEdit::CloseWithLine {
            path: set.iter().next().unwrap().key().clone(),
        }
        .in_ledger_form();
        let json = serde_json::to_value(&edit).unwrap();
        assert_eq!(serde_json::from_value::<DrawingEdit>(json).unwrap(), edit);
    }
}
