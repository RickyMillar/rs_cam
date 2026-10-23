//! The selection refusal and the selection note (DESIGN 5.1, lead ruling
//! D2/D13).
//!
//! [`super::view`](fn@super::view) turns a resolved selection into a refusal or into the
//! items of the op. A refusal names the item, the cause and the fix. A note
//! counts the items that the default rule matched and that the op cannot
//! cut: the op cuts the rest, and the note says what it left out.

use std::borrow::Cow;
use std::fmt;

use crate::compute::catalog::ItemUse;
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::ModelId;

use super::extend::{CannotExtend, PathSide};
use super::hole_fit::{DrillSize, HoleFit, HoleFitRule, hole_fit};
use super::item::{ItemKind, SourceRef};
use super::key::MatchKey;
use super::resolve::{ResolvedSelection, SelectionFrame};

/// The name of one item in a refusal or a note: its match key, which holds
/// the source, the kind, the layer, the anchor and the measure. It is the
/// key of the item before the frame door, so it names the item as the
/// drawing holds it (and as a viewport highlight finds it).
///
/// The key is boxed: since UX-7 a key holds a Z, and an unboxed key makes
/// `SelectionRefusal` larger than the `clippy::result_large_err` limit of
/// every `Result<_, SelectionRefusal>`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRef {
    pub key: Box<MatchKey>,
}

impl ItemRef {
    pub fn new(key: MatchKey) -> Self {
        Self { key: Box::new(key) }
    }
}

/// The noun of a kind, singular and plural. The GUI selection section
/// (P4.3) prints its counts with it, so a count and a refusal use one noun.
pub fn kind_noun(kind: ItemKind, count: usize) -> &'static str {
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
        ItemUse::RegionsExtendOpen {
            side: PathSide::Left,
        } => {
            "cuts closed areas (closed loops, circles, faces), and each open path extended \
             to the stock on its left side"
        }
        ItemUse::RegionsExtendOpen {
            side: PathSide::Right,
        } => {
            "cuts closed areas (closed loops, circles, faces), and each open path extended \
             to the stock on its right side"
        }
        ItemUse::Curves => "follows lines only (closed loops, circles, open paths, face outlines)",
        ItemUse::Edges => {
            "bevels closed areas of a drawing (closed loops, circles, faces) and the convex top \
             edges of a STEP model only"
        }
        ItemUse::Holes {
            fit: HoleFitRule::None,
            ..
        } => "drills points and circles only",
        ItemUse::Holes {
            fit: HoleFitRule::ToolSize,
            ..
        } => "drills points and circles of the tool size only",
        ItemUse::Holes {
            fit: HoleFitRule::LargerThanTool,
            ..
        } => "mills only circles larger than the tool (up to two tool diameters)",
    }
}

/// How a hole that is not of the tool size compares with the tool, as a
/// phrase after "its diameter" (geometry selection P6.5). A size tolerance
/// wider than the default ([`DrillSize::has_wide_tolerance`]) is named in
/// the phrase of a larger and a smaller circle (operator ruling
/// 2026-10-22).
fn hole_fit_phrase(fit: HoleFit, size: &DrillSize) -> String {
    let d = size.diameter;
    let beyond = if size.has_wide_tolerance() {
        format!(" by more than the size tolerance {:.3} mm", size.tolerance)
    } else {
        String::new()
    };
    match fit {
        HoleFit::Larger => format!("is larger than the tool diameter {d:.3} mm{beyond}"),
        HoleFit::LeavesCore => {
            format!("is more than two times the tool diameter {d:.3} mm")
        }
        HoleFit::Smaller => format!("is smaller than the tool diameter {d:.3} mm{beyond}"),
        HoleFit::NoSize | HoleFit::ToolSize => {
            format!("is the tool diameter {d:.3} mm")
        }
    }
}

/// The fix for a circle that an op of `rule` does not cut (geometry
/// selection P6.5, ruling O-3; P6.6 names the Bore op). The match names
/// every rule and every fit.
fn hole_fit_fix(rule: HoleFitRule, fit: HoleFit) -> &'static str {
    match (rule, fit) {
        (HoleFitRule::ToolSize, HoleFit::Larger) => {
            "To cut it, add a Bore op for it, or use a tool of the circle size."
        }
        (HoleFitRule::ToolSize | HoleFitRule::LargerThanTool, HoleFit::LeavesCore) => {
            "To cut it, use a Pocket on the circle or a larger tool."
        }
        (HoleFitRule::ToolSize, HoleFit::Smaller) => "To cut it, use a tool of the circle size.",
        (HoleFitRule::LargerThanTool, HoleFit::Smaller) => {
            "To cut it, use a smaller tool, or a Drill with a tool of the circle size."
        }
        (HoleFitRule::LargerThanTool, HoleFit::ToolSize) => "To cut it, drill it with a Drill op.",
        // An op of the rule cuts these fits, so no note or refusal names
        // them.
        (HoleFitRule::ToolSize, HoleFit::NoSize | HoleFit::ToolSize)
        | (HoleFitRule::LargerThanTool, HoleFit::NoSize | HoleFit::Larger)
        | (
            HoleFitRule::None,
            HoleFit::NoSize
            | HoleFit::ToolSize
            | HoleFit::Larger
            | HoleFit::LeavesCore
            | HoleFit::Smaller,
        ) => "",
    }
}

/// The rule of a hole op, from its item use. A use that is not `Holes`
/// has no fit rule ([`HoleFitRule::None`]).
fn fit_rule(item_use: ItemUse) -> HoleFitRule {
    match item_use {
        ItemUse::Holes { fit, .. } => fit,
        ItemUse::None
        | ItemUse::Regions
        | ItemUse::RegionsExtendOpen { .. }
        | ItemUse::Curves
        | ItemUse::Edges => HoleFitRule::None,
    }
}

/// Why a reader of `item_use` cannot cut items of `kind`: "The op cuts
/// closed areas only (...), so it does not cut open paths." The GUI pick
/// bar (geometry selection P4.5) shows it on a disabled kind chip. It
/// reads [`use_phrase`], the phrase of the `KindNotCut` refusal, so the
/// chip and the refusal give one reason.
pub fn kind_not_cut_reason(kind: ItemKind, item_use: ItemUse) -> String {
    format!(
        "The op {}, so it does not cut {}.",
        use_phrase(item_use),
        kind_noun(kind, 2)
    )
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
    /// Why the op does not cut the items.
    pub cause: NoteCause,
}

/// Why an op leaves an item of its selection out and cuts the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteCause {
    /// The default rule matched a kind that the op cannot cut (D2/D13).
    KindNotServed,
    /// Geometry selection P3.7 (lead ruling): under the floor "each picked
    /// face", the floor of this face is at or above the Top Z, so it has
    /// nothing to cut. The op cuts the other faces; it refuses
    /// `FloorAboveTop` only when every face is at the top.
    AtTheTop {
        /// The floor Z of the face, in the setup frame.
        floor_z: f64,
        /// The resolved Top Z of the op, in the setup frame.
        top_z: f64,
    },
    /// Geometry selection P6.5 (operator ruling O-3): the circle is not of
    /// the tool size, so a Drill does not make its hole. One note for each
    /// circle (lead ruling L-18).
    HoleFit {
        /// How the circle compares with the tool (never a fit of the tool
        /// size).
        fit: HoleFit,
        /// The tool diameter and the size tolerance of the op.
        size: DrillSize,
    },
    /// Chamfer Clearance (operator ruling 2026-10-23, the Fusion default):
    /// an edge of a Chamfer ends at a higher wall, so the cut stops short
    /// of the wall and the V-bit does not cut into it
    /// (`compute::edge_clear`). The op cuts the rest of the edge. `count`
    /// is the number of edges; `first` names the first of them.
    StopsShort {
        /// The number of edge ends that stop short.
        ends: usize,
        /// The shortest and the longest stop (mm), along the edge.
        min_mm: f64,
        max_mm: f64,
    },
    /// Chamfer Clearance: the V-bit reaches a higher wall along all of the
    /// edge, so the op does not cut it (`compute::edge_clear`).
    AtAWall,
    /// "Cut past open sides" (operator ruling 2026-10-23): the setting is
    /// on, and no side of any face of the op is open (a wall stands at
    /// each side), so the pocket stays inside each face
    /// (`compute::open_sides`). The op cuts every face; this is not a
    /// refusal. `count` is the number of faces; `first` names the first.
    NoOpenSide,
    /// R-A (operator ruling 2026-10-23, the Kiri:Moto and FreeCAD
    /// default): a Profile Outside cuts the part outline of a mesh model to
    /// the model bottom or below with no tabs, so the last level frees the
    /// part. The op cuts; this is not a refusal (`PART_FREE_TEXT`).
    FreesThePart,
}

/// The text of a [`NoteCause::FreesThePart`] note (R-A, operator ruling
/// 2026-10-23).
pub const PART_FREE_TEXT: &str =
    "This cut frees the part from the stock. Add tabs if the part must stay held.";

/// The text of a [`NoteCause::NoOpenSide`] note (operator ruling
/// 2026-10-23).
pub const NO_OPEN_SIDE_TEXT: &str =
    "No side of the face is open; the pocket stays inside the face.";

impl fmt::Display for SelectionNote {
    /// "2 open paths on layer 'TEXT' are not cut: the op cuts closed areas
    /// only (...). The first: open path (...)." An open path also gets the
    /// fix that suits the op ([`note_fix`]). P3.7: "1 face is not cut: it
    /// is at the top (...). The face: face (...)."
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = if self.count == 1 { "is" } else { "are" };
        if let NoteCause::HoleFit { fit, size } = self.cause {
            return write!(
                f,
                "{} {}{} {verb} not cut: its diameter {}, and the op {}. The {}: {}. {}",
                self.count,
                kind_noun(self.kind, self.count),
                layer_phrase(self.layer.as_deref()),
                hole_fit_phrase(fit, &size),
                use_phrase(self.item_use),
                kind_noun(self.kind, 1),
                self.first,
                hole_fit_fix(fit_rule(self.item_use), fit)
            );
        }
        if let NoteCause::StopsShort {
            ends,
            min_mm,
            max_mm,
        } = self.cause
        {
            let (end_word, stop_word) = if ends == 1 {
                ("end", "stops")
            } else {
                ("ends", "stop")
            };
            let by = if max_mm - min_mm < crate::selection::CAD_ROUNDING_MM {
                format!("{max_mm:.3} mm")
            } else {
                format!("{min_mm:.3} to {max_mm:.3} mm")
            };
            return write!(
                f,
                "{ends} edge {end_word} {stop_word} short of a wall by {by}, so the V-bit does not \
                 cut into the wall. The first edge: {}.",
                self.first
            );
        }
        if self.cause == NoteCause::NoOpenSide {
            return f.write_str(NO_OPEN_SIDE_TEXT);
        }
        if self.cause == NoteCause::FreesThePart {
            return f.write_str(PART_FREE_TEXT);
        }
        if self.cause == NoteCause::AtAWall {
            return write!(
                f,
                "{} {}{} {verb} not cut: the V-bit reaches a wall along all of it. The first: {}. \
                 To bevel it, use a V-bit with a smaller diameter.",
                self.count,
                kind_noun(self.kind, self.count),
                layer_phrase(self.layer.as_deref()),
                self.first
            );
        }
        if let NoteCause::AtTheTop { floor_z, top_z } = self.cause {
            return write!(
                f,
                "{} {}{} {verb} not cut: it is at the top, so it has nothing to cut (its \
                 floor Z {floor_z:.3} is at or above the Top height {top_z:.3}). The face: {}. \
                 To cut it, set a larger depth in the Bottom height row or pin the Top height \
                 above it.",
                self.count,
                kind_noun(self.kind, self.count),
                layer_phrase(self.layer.as_deref()),
                self.first
            );
        }
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
        (ItemKind::Path, ItemUse::Regions) => Some(
            "Use Trace or Project Curve for an open path (or Profile with side On, or Pocket \
             with Extend to stock), or close it in the drawing.",
        ),
        (ItemKind::Path, ItemUse::Holes { .. }) => {
            Some("To cut the open path, add a Trace or Project Curve operation.")
        }
        // R-C: a drawing path states no material side, so a Chamfer does
        // not bevel it.
        (ItemKind::Path, ItemUse::Edges) => {
            Some("To bevel along an open path, close it in the drawing, or use Trace with a V-bit.")
        }
        // An op that follows lines cuts an open path, an op that extends
        // an open path to the stock cuts it (P6.4), and an op that reads no
        // item skips nothing: none gives a path note.
        (ItemKind::Path, ItemUse::Curves | ItemUse::RegionsExtendOpen { .. } | ItemUse::None) => {
            None
        }
        (
            ItemKind::Loop | ItemKind::Point | ItemKind::Circle | ItemKind::Face,
            ItemUse::Regions
            | ItemUse::RegionsExtendOpen { .. }
            | ItemUse::Curves
            | ItemUse::Edges
            | ItemUse::Holes { .. }
            | ItemUse::None,
        ) => None,
    }
}

/// Why a model edge does not serve a Chamfer (R-C, operator ruling
/// 2026-10-23, SAFETY). The STEP door states the form of each edge
/// (`selection::EdgeForm`, in the model frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeRefusal {
    /// A wall rises above the edge (a concave edge). A V-bit along it cuts
    /// into that wall.
    Concave,
    /// The material is not below the edge on one side only: a bottom edge,
    /// an edge between two walls, or an edge whose faces the STEP door
    /// could not read.
    NotATopEdge,
    /// The setup turns the model (a Bottom or a side setup). The edge form
    /// is a fact of the model frame, so it does not hold in this setup.
    TurnedSetup,
}

impl EdgeRefusal {
    /// The cause as a phrase.
    const fn phrase(self) -> &'static str {
        match self {
            Self::Concave => {
                "this edge is concave: a wall rises above it, so a V-bit along it cuts into \
                 that wall"
            }
            Self::NotATopEdge => {
                "this edge is not a convex top edge: the material is not below it on one \
                 side only"
            }
            Self::TurnedSetup => {
                "the setup turns the model, so an edge of the model is not a top edge in \
                 this setup"
            }
        }
    }

    /// The fix.
    const fn fix(self) -> &'static str {
        match self {
            Self::Concave | Self::NotATopEdge => {
                "Remove the edge from the selection, and pick the convex top edges to chamfer"
            }
            Self::TurnedSetup => {
                "Chamfer the edges of the model in a Top setup, or pick closed loops of a \
                 drawing"
            }
        }
    }
}

/// Why a selection refuses (DESIGN 5.1).
#[derive(Debug, Clone, PartialEq)]
pub enum RefusalCause {
    /// The op has nothing to cut. `skipped` holds the items the default
    /// rule matched and the op cannot cut (empty when the rules matched
    /// nothing at all). `rims` (F29) holds the convex top circular edges of
    /// a STEP model that fit the tool of a Drill and that the default rule
    /// did not take (R-B: it takes the hole rims with a depth only), so the
    /// text names them
    /// ([`SelectionRefusal::with_default_rims`]). Empty for every other
    /// op.
    Empty {
        skipped: Vec<SelectionNote>,
        rims: Vec<ItemRef>,
    },
    /// No item matches a stored pick (the drawing changed).
    LostPick,
    /// No item matches the face key of the floor "A picked face"
    /// (geometry selection UX-8): the model changed.
    FloorFaceLost,
    /// Two or more items match one stored pick (a duplicate entity).
    AmbiguousPick { matches: usize },
    /// The user named an item (a pick, or a rule that names its kind) that
    /// the op cannot cut: an open path in a closed-area op, and so on.
    KindNotCut { kind: ItemKind, item_use: ItemUse },
    /// A picked face that is not a flat horizontal face (P2.4): a BREP
    /// face with no Face item (`face` is its BREP id; the refusal has no
    /// item), or a Face item that the setup turns on its side (the item
    /// names it).
    FaceNotFlat { face: Option<FaceGroupId> },
    /// A face in the closed areas of an op that points down in the setup
    /// frame (geometry selection P3.4, lead ruling L-9). Its material is
    /// above its plane, so the tool cannot stand on it and its plane is not
    /// a floor. The same face points up in a flipped setup and cuts there.
    /// A machining boundary keeps a down face (`view_boundary`).
    FaceFacesDown,
    /// The floor of the op is "each picked face" (geometry selection P3.5),
    /// and the exterior of one of its closed areas is not a face: a loop, a
    /// circle or the stock ring. That area has no plane Z, so it has no
    /// floor.
    NoFaceForFloor,
    /// The floor of one closed area under "each picked face" is at or above
    /// the resolved Top Z of the op (geometry selection P3.5, ruling L-16):
    /// its ladder would be empty.
    FloorAboveTop {
        /// The floor Z of the area, in the setup frame.
        floor_z: f64,
        /// The resolved Top Z of the op, in the setup frame.
        top_z: f64,
    },
    /// Geometry selection P6.5 (operator ruling O-3): the user named a
    /// circle that the op does not cut by its size: a Drill drills holes of
    /// the tool size only, a Bore (P6.6) mills circles larger than the tool
    /// up to two tool diameters. Under the default rule the op leaves such
    /// a circle out with a note ([`NoteCause::HoleFit`]).
    HoleDoesNotFit {
        /// How the circle compares with the tool.
        fit: HoleFit,
        /// The tool diameter and the size tolerance of the op.
        size: DrillSize,
        /// The fit rule of the op: it gives the text and the fix.
        rule: HoleFitRule,
    },
    /// Geometry selection P6.4 (operator ruling O-2): the op extends each
    /// open path to the stock (Pocket with Extend to stock), and this path
    /// cannot extend (`P6_DESIGN.md` 3.4). Under the default rule it
    /// refuses too: the op serves the kind (lead ruling L-10).
    CannotExtend { why: CannotExtend },
    /// R-C (operator ruling 2026-10-23, SAFETY): a Chamfer names a model
    /// edge (a chain, a circular edge, a face ring or an up face) that is
    /// not a convex top edge, or a model edge in a turned setup. The item
    /// names the edge. The default rule of a Chamfer takes the convex top
    /// chains only, so it never gives a concave edge.
    NotAConvexEdge { why: EdgeRefusal },
    /// R-B: a pick names the floor circle of a blind hole. A Drill drills
    /// a hole from its rim, so the floor circle does not serve. A viewport
    /// click and an MCP `near` pick map a floor circle to its rim
    /// (`ItemSet::rim_of`), so only a pick by key gives this cause.
    HoleFloorNotRim,
    /// R-B: a Drill names a hole of a STEP model in a setup that turns the
    /// model (a Bottom or a side setup). The rim and the depth of a hole
    /// are facts of the model frame, so they do not hold in this setup.
    HoleInTurnedSetup,
    /// R-A (operator ruling 2026-10-23, SAFETY): a pick names the part
    /// outline of a mesh model, and the op cuts an area or an edge of it
    /// (`OutlineUse::Refuse`). The outline is the whole part, so the op
    /// would cut the part away.
    PartOutlineIsThePart,
    /// R-B: the floor of the Drill is "each hole floor", and this hole has
    /// no depth: a point or a circle of a drawing, or a circle of a model
    /// with no floor circle below it.
    NoHoleDepth,
    /// A rule or a pick names a model that does not exist. No fallback to
    /// another model.
    MissingSource(SourceRef),
    /// The selection names a face of a mesh model whose flat regions are
    /// not computed yet (geometry selection P4.7). Generation computes them
    /// before it resolves, so it does not give this cause; a resolution
    /// that did not compute them (the `selection_status` read on the frame
    /// loop) does.
    FacesNotReady { model: ModelId },
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

    /// F29: the same refusal, with the top rims of the holes that fit
    /// `size` and that the default rule of `resolved` did not take
    /// ([`ResolvedSelection::rims_off_default`]). Only an `Empty` refusal
    /// of a Drill (`HoleFitRule::ToolSize`) changes; the selection does not
    /// change (ruling R-B is open).
    #[must_use]
    pub fn with_default_rims<F: SelectionFrame>(
        mut self,
        resolved: &ResolvedSelection<F>,
        item_use: ItemUse,
        size: &DrillSize,
    ) -> Self {
        let ItemUse::Holes {
            fit: HoleFitRule::ToolSize,
            ..
        } = item_use
        else {
            return self;
        };
        if let RefusalCause::Empty { rims, .. } = &mut self.cause {
            *rims = resolved
                .rims_off_default()
                .iter()
                .filter(|key| hole_fit(Some(key.measure), size) == HoleFit::ToolSize)
                .map(|key| ItemRef::new(key.clone()))
                .collect();
        }
        self
    }

    /// The fix: one text per cause.
    pub fn fix(&self) -> Cow<'static, str> {
        if let RefusalCause::Empty { rims, .. } = &self.cause
            && let Some(first) = rims.first()
        {
            return Cow::Owned(format!(
                "Pick the top circle of each hole, or add the rule 'all circles Ø{:.3}'",
                first.key.measure
            ));
        }
        Cow::Borrowed(self.static_fix())
    }

    /// The fix of each cause that states one text.
    fn static_fix(&self) -> &'static str {
        match &self.cause {
            RefusalCause::Empty { .. } => {
                "Add items that the operation can cut to its selection, or use \
                 an operation that cuts the items of the drawing"
            }
            RefusalCause::LostPick => "Pick the item again, or remove the pick from the selection",
            RefusalCause::FloorFaceLost => {
                "Use another face as the floor (Geometry tab, Use as floor), or set the Bottom \
                 height from another reference (Heights tab)"
            }
            RefusalCause::AmbiguousPick { .. } => {
                "Remove the duplicate entity from the drawing, or remove the \
                 pick and select the item by a rule"
            }
            RefusalCause::KindNotCut { .. } => {
                "Remove the item from the selection, or use an operation that \
                 cuts this kind of item (Trace or Project Curve for an open \
                 path, Drill for a point)"
            }
            RefusalCause::FaceNotFlat { .. } => {
                "Pick a flat horizontal face, or remove the face from the selection"
            }
            RefusalCause::FaceFacesDown => {
                "Pick it in a setup that faces it, or pick the face above it"
            }
            RefusalCause::NoFaceForFloor => {
                "Pick faces only, or set the Bottom height from another reference (the Top \
                 height, the stock, the model or a Z value)"
            }
            RefusalCause::FloorAboveTop { .. } => {
                "Set a larger depth in the Bottom height row, pin the Top height above the \
                 face, or remove the face from the selection"
            }
            RefusalCause::CannotExtend { why } => why.fix(),
            RefusalCause::NotAConvexEdge { why } => why.fix(),
            RefusalCause::HoleFloorNotRim => {
                "Pick the rim circle of the hole (the circle at the top of the hole), or remove \
                 the floor circle from the selection"
            }
            RefusalCause::PartOutlineIsThePart => {
                "Use a Profile with side Outside to cut the part outline, or remove the outline \
                 from the selection"
            }
            RefusalCause::HoleInTurnedSetup => {
                "Drill the holes of the model in a Top setup, or pick points or circles of a \
                 drawing"
            }
            RefusalCause::NoHoleDepth => {
                "Set the Bottom height from another reference (the Top height, the stock, the \
                 model or a Z value), or remove the hole from the selection"
            }
            RefusalCause::HoleDoesNotFit { fit, rule, .. } => match (*rule, *fit) {
                (HoleFitRule::LargerThanTool, HoleFit::ToolSize) => {
                    "Drill it with a Drill op, or remove the circle from the selection"
                }
                (HoleFitRule::LargerThanTool, HoleFit::Smaller) => {
                    "Use a smaller tool, use a Drill with a tool of the circle size, or remove \
                     the circle from the selection"
                }
                (_, HoleFit::LeavesCore) => {
                    "Use a Pocket on the circle or a larger tool, or remove the circle from the \
                     selection"
                }
                (_, HoleFit::Smaller) => {
                    "Use a tool of the circle size, or remove the circle from the selection"
                }
                (_, HoleFit::NoSize | HoleFit::ToolSize | HoleFit::Larger) => {
                    "Add a Bore op for it, use a tool of the circle size, or remove the circle \
                     from the selection"
                }
            },
            RefusalCause::MissingSource(_) => {
                "Set an existing model on the toolpath, import the model again, \
                 or remove the rule or the pick that names it"
            }
            RefusalCause::UnknownLayer(_) => {
                "Correct the layer name in the rule, or add the layer to the drawing"
            }
            RefusalCause::NoSuchRule { .. } => "Name a rule that the selection holds",
            RefusalCause::FacesNotReady { .. } => {
                "Wait until the faces are computed; Generate computes them itself"
            }
        }
    }

    /// The cause as a phrase.
    fn cause_text(&self) -> String {
        match &self.cause {
            RefusalCause::Empty { skipped, rims } if skipped.is_empty() && rims.is_empty() => {
                "the selection holds no item to cut".to_owned()
            }
            RefusalCause::Empty { skipped, rims } => {
                let mut notes: Vec<String> = skipped.iter().map(ToString::to_string).collect();
                if !rims.is_empty() {
                    notes.push(rims_text(rims));
                }
                // Each note ends with a full stop, and the refusal adds
                // its own before "Fix:", so the last one goes (no "..").
                let notes = notes.join(" ");
                let notes = notes.strip_suffix('.').unwrap_or(&notes);
                format!("nothing is left to cut. {notes}")
            }
            RefusalCause::LostPick => "no item of the drawing matches this pick".to_owned(),
            RefusalCause::FloorFaceLost => {
                "the floor is \"a picked face\", and no item of the model matches that face"
                    .to_owned()
            }
            RefusalCause::AmbiguousPick { matches } => {
                format!("{matches} items of the drawing match this pick")
            }
            RefusalCause::KindNotCut { kind, item_use } => format!(
                "the selection names a {}, and the op {}",
                kind_noun(*kind, 1),
                use_phrase(*item_use)
            ),
            RefusalCause::FaceNotFlat { face: Some(id) } => format!(
                "the selection picks face {} of the model, which is not flat and horizontal",
                id.0
            ),
            RefusalCause::FaceNotFlat { face: None } => {
                "the face is not flat and horizontal in the setup frame".to_owned()
            }
            RefusalCause::FaceFacesDown => "this face faces down in this setup".to_owned(),
            RefusalCause::NoFaceForFloor => {
                "the floor is \"each picked face\", and this closed area is not a face, so it \
                 has no floor"
                    .to_owned()
            }
            RefusalCause::FloorAboveTop { floor_z, top_z } => format!(
                "the floor of this face, Z {floor_z:.3}, is at or above the Top height \
                 {top_z:.3}, so it has nothing to cut"
            ),
            RefusalCause::HoleDoesNotFit { fit, size, rule } => format!(
                "the diameter of this circle {}, and the op {}",
                hole_fit_phrase(*fit, size),
                use_phrase(ItemUse::Holes {
                    may_be_empty: false,
                    fit: *rule,
                })
            ),
            RefusalCause::CannotExtend { why } => {
                format!("the open path cannot extend to the stock: {}", why.phrase())
            }
            RefusalCause::NotAConvexEdge { why } => why.phrase().to_owned(),
            RefusalCause::HoleFloorNotRim => {
                "this circle is the floor of a hole, and a Drill drills a hole from its rim"
                    .to_owned()
            }
            RefusalCause::PartOutlineIsThePart => {
                "this item is the part outline, which is the whole part, and this operation cuts \
                 inside it or on it, so it cuts the part away"
                    .to_owned()
            }
            RefusalCause::HoleInTurnedSetup => {
                "the setup turns the model, so the rim of this hole is not at its top in this \
                 setup"
                    .to_owned()
            }
            RefusalCause::NoHoleDepth => {
                "the floor is \"each hole floor\", and this hole has no known depth".to_owned()
            }
            RefusalCause::MissingSource(source) => {
                let name = match source {
                    SourceRef::ToolpathModel => "the model of the toolpath".to_owned(),
                    SourceRef::Model(id) => format!("model {}", id.raw()),
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
            RefusalCause::FacesNotReady { model } => {
                format!(
                    "the flat faces of model {} are not computed yet",
                    model.raw()
                )
            }
        }
    }
}

/// F29: "The default takes the rims of the holes with a known depth only.
/// This model has 1 circle that fits the tool: Ø6.000 at X 15.000 Y 15.000
/// Z 20.000." Each rim at its centre and Z, in the model frame (the frame
/// of the item name). R-B: the default of a Drill takes each hole rim
/// with a depth, so the list holds the convex top circles with no floor
/// circle below them.
fn rims_text(rims: &[ItemRef]) -> String {
    let at: Vec<String> = rims
        .iter()
        .map(|rim| {
            let k = &rim.key;
            let z = k.z.map_or_else(String::new, |z| format!(" Z {z:.3}"));
            format!(
                "Ø{:.3} at X {:.3} Y {:.3}{z}",
                k.measure, k.anchor[0], k.anchor[1]
            )
        })
        .collect();
    let (noun, verb) = if rims.len() == 1 {
        ("circle", "fits")
    } else {
        ("circles", "fit")
    };
    format!(
        "The default takes the rims of the holes with a known depth only. This model has {} \
         {noun} that {verb} the tool: {}.",
        rims.len(),
        at.join("; ")
    )
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
