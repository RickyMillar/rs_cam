//! The stored selection: rules, picks and excludes (DESIGN 3.2, lead ruling
//! D2/D13).
//!
//! A [`Selection`] says WHICH items an op cuts. It does not say how: the
//! op's `ItemUse` decides that in [`super::view`](fn@super::view). A selection is pure
//! data; [`super::resolve`](fn@super::resolve) matches it against the items of the models.
//!
//! `Selection` has NO `impl Default`. Each op states its own default
//! ([`Selection::drawing`], [`Selection::empty`] or [`Selection::stock`]),
//! so a file with no `selection` key loads as the default of THAT op, never
//! as the default of another op.

use serde::{Deserialize, Serialize};

use crate::compute::catalog::{ItemUse, OutlineUse};
use crate::ids::ModelId;

use super::hole_fit::HoleFitRule;
use super::item::{Item, ItemKind, Shape, SourceRef};
use super::key::MatchKey;
use super::refusal::{RefusalCause, SelectionRefusal};

/// Which kinds a rule matches.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KindFilter {
    /// The kinds that the op's `ItemUse` serves for its current config,
    /// among the drawing kinds (loop, path, point, circle). A face is not a
    /// drawing kind: it enters a selection only by a pick or by a rule that
    /// names it (`Only([Face])`). On a STEP model the default rule selects
    /// the loops only (the face rings, UX-7 / ruling Q3,
    /// [`served_by_default`]), so a Drill there has nothing to cut; a
    /// Chamfer (`ItemUse::Edges`, R-C) takes the convex top chains.
    ///
    /// `resolve` does not know the op, so it matches every drawing kind
    /// here and marks the match [`super::MatchedBy::Default`]. `view` then
    /// leaves out, with a note, each item the op cannot cut.
    #[default]
    Served,
    /// These kinds only. A rule with `Only` names its kinds, so an item it
    /// matches that the op cannot cut refuses (`KindNotCut`).
    Only(Vec<ItemKind>),
}

/// A circle diameter range (S2): `nominal_mm` plus or minus
/// `tolerance_mm`, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DiameterRange {
    pub nominal_mm: f64,
    pub tolerance_mm: f64,
}

impl DiameterRange {
    pub fn contains(&self, diameter: f64) -> bool {
        (diameter - self.nominal_mm).abs() <= self.tolerance_mm
    }
}

/// Which way a face points, in the SETUP frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facing {
    /// The normal has a positive Z part.
    Up,
    /// The normal has a negative Z part.
    Down,
    /// Either way.
    Either,
}

impl Facing {
    /// True when a face with the normal Z part `normal_z` (SETUP frame)
    /// points this way. The one facing rule: the face filter, the auto top
    /// (`face_top_z`), the `FaceFacesDown` refusal and the facing counts
    /// of the `selection_status` report read it (geometry selection P3.4).
    pub fn admits(self, normal_z: f64) -> bool {
        match self {
            Facing::Up => normal_z > 0.0,
            Facing::Down => normal_z < 0.0,
            Facing::Either => true,
        }
    }
}

/// The faces of a selection by facing in the SETUP frame
/// (`SelectionReport::faces_by_facing`, geometry selection P3.4). A face
/// that a lateral setup turns on its side is in neither count.
///
/// The wire spelling: `{"up": 3, "down": 1}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacesByFacing {
    /// The faces whose normal points up ([`Facing::Up`]).
    pub up: usize,
    /// The faces whose normal points down ([`Facing::Down`]).
    pub down: usize,
}

impl FacesByFacing {
    /// Count one face with the normal Z part `normal_z` (SETUP frame), by
    /// [`Facing::admits`].
    pub fn count(&mut self, normal_z: f64) {
        if Facing::Up.admits(normal_z) {
            self.up += 1;
        } else if Facing::Down.admits(normal_z) {
            self.down += 1;
        }
    }
}

/// A Z range in mm, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZRange {
    pub min: f64,
    pub max: f64,
}

impl ZRange {
    pub fn contains(&self, z: f64) -> bool {
        z >= self.min && z <= self.max
    }
}

/// A face filter: the facing and the plane Z, both in the SETUP frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceFilter {
    pub facing: Facing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<ZRange>,
}

/// The test of one rule. Every part must admit the item.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Empty = every layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<String>,
    #[serde(default)]
    pub kinds: KindFilter,
    /// Circles only: a filter with a diameter range admits no other kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<DiameterRange>,
    /// Faces only: a filter with a face filter admits no other kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceFilter>,
    /// Model edge items only (geometry selection GT-0b): the item Z
    /// ([`Item::z`]) in the SETUP frame, in this range. A filter with a Z
    /// range admits no drawing item, no stock item and no face (a face
    /// takes `FaceFilter::z`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<ZRange>,
}

impl Filter {
    /// True when the filter admits `item`. `face_frame` gives the plane Z
    /// and the normal Z part of a face, and the Z of a model edge item, in
    /// the setup frame (`resolve::face_in_setup_frame`,
    /// `resolve::item_z_in_setup_frame`).
    ///
    /// `(item_use, outline_use)` is the use of the op that reads the
    /// selection: the default rule ([`KindFilter::Served`]) reads it on a
    /// model edge item and on the part outline ([`served_by_default`]).
    pub(super) fn admits(
        &self,
        item: &Item,
        (item_use, outline_use): (ItemUse, OutlineUse),
        face_frame: impl Fn(&Item) -> (f64, f64),
    ) -> bool {
        let layer_ok = self.layers.is_empty()
            || item
                .layer()
                .is_some_and(|l| self.layers.iter().any(|want| want == l));
        let kind = item.kind();
        let kind_ok = match &self.kinds {
            KindFilter::Served => served_by_default(item, item_use, outline_use),
            KindFilter::Only(kinds) => kinds.contains(&kind),
        };
        let diameter_ok = match (&self.diameter, item.shape()) {
            (None, _) => true,
            (Some(range), Shape::Circle { diameter, .. }) => range.contains(*diameter),
            (Some(_), _) => false,
        };
        let face_ok = match (&self.face, item.shape()) {
            (None, _) => true,
            (Some(filter), Shape::Face(_)) => {
                let (plane_z, normal_z) = face_frame(item);
                filter.facing.admits(normal_z) && filter.z.is_none_or(|z| z.contains(plane_z))
            }
            (Some(_), _) => false,
        };
        let z_ok = match (&self.z, item.z()) {
            (None, _) => true,
            (Some(range), Some(_)) => range.contains(face_frame(item).0),
            (Some(_), None) => false,
        };
        layer_ok && kind_ok && diameter_ok && face_ok && z_ok
    }
}

/// The items that the default rule (`KindFilter::Served`) selects, for an
/// op of `item_use`.
///
/// - A drawing item (no Z): every drawing kind ([`is_drawing_kind`]).
/// - A model edge item of a STEP model (geometry selection UX-7, operator
///   ruling Q3 "default to face rings (loops) unless there is a selection
///   for edges"): the loops only. A face ring and a closed edge chain that
///   is not a ring are loops; an open edge chain, a circular edge and a
///   vertex enter by a pick or a rule that names them.
/// - R-C (operator ruling 2026-10-23, SAFETY; a departure from Q3 for
///   Chamfer only): an op of [`ItemUse::Edges`] takes the CONVEX TOP edge
///   chains and circular edges ([`super::ModelEdge::is_convex_top`]), not
///   the face rings. A face ring holds concave edges too (the Z 20 ring of
///   the stepped block meets the Z 30 wall), and a V-bit along a concave
///   edge cuts into the wall above it. The convex runs of each ring are
///   its chains.
/// - R-B (operator ruling 2026-10-23): a Drill ([`HoleFitRule::ToolSize`])
///   takes the RIM circle of each hole that has a depth
///   ([`Item::rim_depth`]), and no ring. The floor circle of a blind hole
///   does not serve. The fit step then keeps the rims of the tool size.
///   The Bore and the Pin Drill keep the Q3 rule.
/// - R-A (operator ruling 2026-10-23, SAFETY): the part outline of a mesh
///   model serves only an op of [`OutlineUse::Default`] (Profile Outside),
///   and that op takes ONLY the outline of a mesh model: no face ring.
///   A drawing item (no Z) does not change: a DXF Profile keeps every loop.
pub(super) fn served_by_default(item: &Item, item_use: ItemUse, outline_use: OutlineUse) -> bool {
    if item.is_part_outline() {
        return outline_use == OutlineUse::Default;
    }
    if outline_use == OutlineUse::Default && item.z().is_some() {
        return false;
    }
    let kind = item.kind();
    if item.z().is_none() {
        return is_drawing_kind(kind);
    }
    match item_use {
        ItemUse::Edges => match kind {
            ItemKind::Loop | ItemKind::Path | ItemKind::Circle => item
                .model_edge()
                .is_some_and(|edge| !edge.ring && edge.is_convex_top()),
            ItemKind::Point | ItemKind::Face => false,
        },
        ItemUse::Holes {
            fit: HoleFitRule::ToolSize,
            ..
        } => match kind {
            ItemKind::Circle => item.rim_depth().is_some(),
            ItemKind::Loop | ItemKind::Path | ItemKind::Point | ItemKind::Face => false,
        },
        ItemUse::None
        | ItemUse::Regions
        | ItemUse::RegionsExtendOpen { .. }
        | ItemUse::Curves
        | ItemUse::Holes { .. } => match kind {
            ItemKind::Loop => true,
            ItemKind::Path | ItemKind::Point | ItemKind::Circle | ItemKind::Face => false,
        },
    }
}

/// The drawing kinds: every kind but `Face`. The match has no wildcard, so
/// a new kind states whether the default rule selects it.
pub(super) fn is_drawing_kind(kind: ItemKind) -> bool {
    match kind {
        ItemKind::Loop | ItemKind::Path | ItemKind::Point | ItemKind::Circle => true,
        ItemKind::Face => false,
    }
}

/// One rule: the items of `source` that `filter` admits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub source: SourceRef,
    pub filter: Filter,
}

impl Rule {
    /// The same rule with its source in the form a toolpath on
    /// `toolpath_model` stores ([`SourceRef::for_toolpath_model`]).
    #[must_use]
    pub fn for_toolpath_model(self, toolpath_model: ModelId) -> Self {
        Self {
            source: self.source.for_toolpath_model(toolpath_model),
            ..self
        }
    }

    /// The rules of a layer pick (geometry selection P4.5, a viewport
    /// shift-click): the items of `source` on `layer` whose kind is in
    /// `kinds`. With `circle`, the circles must also be in that diameter
    /// range. It is [`Self::kind_rules`] with one layer.
    pub fn layer_rules(
        source: SourceRef,
        layer: &str,
        kinds: &[ItemKind],
        circle: Option<DiameterRange>,
    ) -> Vec<Rule> {
        Self::kind_rules(source, &[layer.to_owned()], kinds, circle)
    }

    /// The rules of the items of `source` on `layers` (every layer when
    /// empty) whose kind is in `kinds`. With `circle`, the circles must
    /// also be in that diameter range.
    ///
    /// A filter with a diameter range admits circles only
    /// ([`Filter::admits`]). So when `kinds` holds circles and other kinds,
    /// the circles get a rule of their own with the range, and the other
    /// kinds get one rule with no range. A `circle` without
    /// [`ItemKind::Circle`] in `kinds` is not used. No kind gives no rule.
    pub fn kind_rules(
        source: SourceRef,
        layers: &[String],
        kinds: &[ItemKind],
        circle: Option<DiameterRange>,
    ) -> Vec<Rule> {
        let rule = |kinds: Vec<ItemKind>, diameter: Option<DiameterRange>| Rule {
            source,
            filter: Filter {
                layers: layers.to_vec(),
                kinds: KindFilter::Only(kinds),
                diameter,
                face: None,
                z: None,
            },
        };
        if kinds.is_empty() {
            return Vec::new();
        }
        let has_circle = kinds.contains(&ItemKind::Circle);
        match circle {
            Some(range) if has_circle => {
                let others: Vec<ItemKind> = kinds
                    .iter()
                    .copied()
                    .filter(|k| *k != ItemKind::Circle)
                    .collect();
                let mut rules = Vec::new();
                if !others.is_empty() {
                    rules.push(rule(others, None));
                }
                rules.push(rule(vec![ItemKind::Circle], Some(range)));
                rules
            }
            Some(_) | None => vec![rule(kinds.to_vec(), None)],
        }
    }

    /// True when the rule names its kinds (`KindFilter::Only`). An item it
    /// matches is then an explicit act of the user.
    pub fn names_kinds(&self) -> bool {
        match self.filter.kinds {
            KindFilter::Served => false,
            KindFilter::Only(_) => true,
        }
    }
}

/// The stored selection of an op.
///
/// The fields are private. The constructors and [`Selection::apply`] are
/// the only writers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// The union of the rule matches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<Rule>,
    /// Items added by hand, matched by geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    picks: Vec<MatchKey>,
    /// Items removed by hand from the rule matches. An exclude does not
    /// remove a pick.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    excludes: Vec<MatchKey>,
}

/// One edit of a selection. [`Selection::apply`] is the one writer.
///
/// No toggle edit exists here: a viewport click must know whether a rule
/// already matches the item, and only the resolved selection knows that.
/// The command (P2.6) resolves first and sends `AddPick` or `RemovePick`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionEdit {
    /// Replace the whole selection.
    Replace(Box<Selection>),
    /// Add a rule. A rule equal to one the selection holds is not added
    /// again. Operator ruling O-PICK: on a selection that is still the op
    /// default ([`Selection::is_default_of`]), the rule replaces the
    /// default.
    AddRule(Rule),
    /// Remove the rule at this index.
    RemoveRule(usize),
    /// Add a pick. It also removes every exclude that names the item.
    /// Operator ruling O-PICK: on a selection that is still the op default
    /// ([`Selection::is_default_of`]), the pick replaces the default, so
    /// the op cuts only the picked item.
    AddPick(MatchKey),
    /// Remove the picks that name the item. When no pick names it, the
    /// edit adds an exclude, so a rule match leaves the selection.
    RemovePick(MatchKey),
    /// Add every key as one edit (box select, SEL-4). O-PICK: on a
    /// selection that is still the op default, the keys replace it.
    /// A key that `matches` an earlier key of the list is dropped.
    AddPicks(Vec<MatchKey>),
    /// Remove every key as one edit. Each key acts as `RemovePick`.
    RemovePicks(Vec<MatchKey>),
    /// Back to the default selection of the op.
    Reset,
}

impl SelectionEdit {
    /// The same edit with each source in the form a toolpath on
    /// `toolpath_model` stores ([`SourceRef::for_toolpath_model`]). The
    /// `edit_toolpath_selection` command applies it, so every surface
    /// stores one spelling for one item (geometry selection P2.7).
    #[must_use]
    pub fn for_toolpath_model(self, toolpath_model: ModelId) -> Self {
        match self {
            SelectionEdit::Replace(selection) => {
                SelectionEdit::Replace(Box::new(selection.for_toolpath_model(toolpath_model)))
            }
            SelectionEdit::AddRule(rule) => {
                SelectionEdit::AddRule(rule.for_toolpath_model(toolpath_model))
            }
            SelectionEdit::AddPick(key) => {
                SelectionEdit::AddPick(key.for_toolpath_model(toolpath_model))
            }
            SelectionEdit::RemovePick(key) => {
                SelectionEdit::RemovePick(key.for_toolpath_model(toolpath_model))
            }
            SelectionEdit::AddPicks(keys) => SelectionEdit::AddPicks(
                keys.into_iter()
                    .map(|k| k.for_toolpath_model(toolpath_model))
                    .collect(),
            ),
            SelectionEdit::RemovePicks(keys) => SelectionEdit::RemovePicks(
                keys.into_iter()
                    .map(|k| k.for_toolpath_model(toolpath_model))
                    .collect(),
            ),
            SelectionEdit::RemoveRule(_) | SelectionEdit::Reset => self,
        }
    }

    /// The edit that adds `rules` to `current`, a selection whose op
    /// default is `op_default`, on a toolpath on `toolpath_model`
    /// (geometry selection P4.3 `+ Rule`, P4.5 shift-click).
    ///
    /// Under the default (a selection that is not empty), the new rules
    /// replace it, so one act gives one set of items: the rule of the first
    /// layer pick of the drill. The two selections are compared in the
    /// stored form ([`Selection::for_toolpath_model`]). Else one rule gives
    /// `AddRule`, and more rules give one `Replace` that appends the rules
    /// the selection does not hold, so the act is one undo step. No rule
    /// gives `None`.
    ///
    /// The viewport and the section call it. An MCP `add_rule` takes the
    /// same rule in [`Selection::apply`] (operator ruling O-PICK).
    pub fn add_rules(
        current: &Selection,
        op_default: &Selection,
        toolpath_model: ModelId,
        mut rules: Vec<Rule>,
    ) -> Option<SelectionEdit> {
        if current.holds_default(op_default, toolpath_model) {
            return (!rules.is_empty())
                .then(|| SelectionEdit::Replace(Box::new(Selection::from_rules(rules))));
        }
        match rules.len() {
            0 => None,
            1 => rules.pop().map(SelectionEdit::AddRule),
            _ => {
                let mut all = current.rules.clone();
                for rule in rules {
                    if !all.contains(&rule) {
                        all.push(rule);
                    }
                }
                Some(SelectionEdit::Replace(Box::new(Selection::from_parts(
                    all,
                    current.picks.clone(),
                    current.excludes.clone(),
                ))))
            }
        }
    }
}

impl Selection {
    /// Every drawing item of the toolpath model that the op can cut: one
    /// rule, [`KindFilter::Served`], every layer.
    pub fn drawing() -> Self {
        Self {
            rules: vec![Rule {
                source: SourceRef::ToolpathModel,
                filter: Filter::default(),
            }],
            picks: Vec::new(),
            excludes: Vec::new(),
        }
    }

    /// No item. The default of the Pin Drill extras.
    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            picks: Vec::new(),
            excludes: Vec::new(),
        }
    }

    /// The stock outline of the setup. The default of Face (decision D5).
    pub fn stock() -> Self {
        Self {
            rules: vec![Rule {
                source: SourceRef::Stock,
                filter: Filter::default(),
            }],
            picks: Vec::new(),
            excludes: Vec::new(),
        }
    }

    /// The closed shapes (loops and circles) of `model`, on every layer:
    /// one rule that names its kinds. The boundary selection that replaced
    /// `BoundarySource::ModelOutline` (geometry selection P2.5).
    pub fn closed_shapes_of(model: ModelId) -> Self {
        Self::from_rules(vec![Rule {
            source: SourceRef::Model(model),
            filter: Filter {
                kinds: KindFilter::Only(vec![ItemKind::Loop, ItemKind::Circle]),
                ..Filter::default()
            },
        }])
    }

    /// The model of a [`Selection::closed_shapes_of`] selection, or `None`
    /// for every other selection.
    pub fn closed_shapes_model(&self) -> Option<ModelId> {
        match self.rules.as_slice() {
            [rule] => match rule.source {
                SourceRef::Model(id) if *self == Self::closed_shapes_of(id) => Some(id),
                SourceRef::Model(_) | SourceRef::ToolpathModel | SourceRef::Stock => None,
            },
            _ => None,
        }
    }

    /// Every source that a rule, a pick or an exclude names, in that
    /// order, with repeats.
    pub fn sources(&self) -> impl Iterator<Item = SourceRef> + '_ {
        self.rules
            .iter()
            .map(|r| r.source)
            .chain(self.picks.iter().map(|k| k.source))
            .chain(self.excludes.iter().map(|k| k.source))
    }

    /// True when no rule, pick or exclude names a model: every source is
    /// [`SourceRef::Stock`], or the selection is empty. Such a selection
    /// resolves with no model (geometry selection P4.2: a Face op on an STL
    /// or with no model still generates).
    pub fn reads_no_model(&self) -> bool {
        self.sources().all(|s| s == SourceRef::Stock)
    }

    /// These picks and no rule.
    pub fn from_picks(picks: Vec<MatchKey>) -> Self {
        Self {
            rules: Vec::new(),
            picks,
            excludes: Vec::new(),
        }
    }

    /// These rules and no pick.
    pub fn from_rules(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            picks: Vec::new(),
            excludes: Vec::new(),
        }
    }

    /// These rules, picks and excludes, as given (a wire or a job file
    /// that holds a whole selection).
    pub fn from_parts(rules: Vec<Rule>, picks: Vec<MatchKey>, excludes: Vec<MatchKey>) -> Self {
        Self {
            rules,
            picks,
            excludes,
        }
    }

    /// The same selection with each source in the form a toolpath on
    /// `toolpath_model` stores ([`SourceRef::for_toolpath_model`]).
    #[must_use]
    pub fn for_toolpath_model(self, toolpath_model: ModelId) -> Self {
        let Self {
            rules,
            picks,
            excludes,
        } = self;
        Self {
            rules: rules
                .into_iter()
                .map(|r| r.for_toolpath_model(toolpath_model))
                .collect(),
            picks: picks
                .into_iter()
                .map(|k| k.for_toolpath_model(toolpath_model))
                .collect(),
            excludes: excludes
                .into_iter()
                .map(|k| k.for_toolpath_model(toolpath_model))
                .collect(),
        }
    }

    /// True when the selection is [`Selection::drawing`] (the serde skip
    /// test of the ops whose default it is).
    pub fn is_drawing(&self) -> bool {
        *self == Self::drawing()
    }

    /// True when the selection is still the op default `op_default`, and
    /// that default is not empty (operator ruling O-PICK). Both
    /// selections must be in the stored form
    /// ([`Selection::for_toolpath_model`]); [`Self::holds_default`]
    /// converts them first.
    ///
    /// An empty default (the Pin Drill extras, a boundary on a project
    /// with no closed ring) is never "the default rule": a first pick on
    /// it adds, as every later pick does.
    pub fn is_default_of(&self, op_default: &Selection) -> bool {
        !op_default.is_empty() && self == op_default
    }

    /// [`Self::is_default_of`] for a toolpath on `toolpath_model`: both
    /// selections are compared in the stored form.
    pub fn holds_default(&self, op_default: &Selection, toolpath_model: ModelId) -> bool {
        self.clone()
            .for_toolpath_model(toolpath_model)
            .is_default_of(&op_default.clone().for_toolpath_model(toolpath_model))
    }

    /// True when the selection holds no rule, no pick and no exclude.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.picks.is_empty() && self.excludes.is_empty()
    }

    /// True when the selection is [`Selection::stock`].
    pub fn is_stock(&self) -> bool {
        *self == Self::stock()
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn picks(&self) -> &[MatchKey] {
        &self.picks
    }

    pub fn excludes(&self) -> &[MatchKey] {
        &self.excludes
    }

    /// The picks of a face (`ItemKind::Face`), in pick order.
    pub fn face_picks(&self) -> impl Iterator<Item = &MatchKey> + '_ {
        self.picks.iter().filter(|k| k.kind == ItemKind::Face)
    }

    /// True when the selection names a face: a face pick, or a rule whose
    /// kinds hold `Face`. A face enters a selection by such an act only
    /// ([`KindFilter::Served`] admits no face).
    pub fn names_faces(&self) -> bool {
        self.face_picks().next().is_some()
            || self.rules.iter().any(|r| match &r.filter.kinds {
                KindFilter::Served => false,
                KindFilter::Only(kinds) => kinds.contains(&ItemKind::Face),
            })
    }

    /// Apply one edit. `op_default` is the default selection of the op
    /// that holds this selection. A selection does not know its op, so the
    /// caller gives it (`OperationConfig::default_selection`, P2.3b), in
    /// the same stored form as `self`.
    ///
    /// Operator ruling O-PICK (2026-10-22): this is THE pick rule of every
    /// surface. While the selection is still the default
    /// ([`Self::is_default_of`]), `AddPick` and `AddRule` replace it: the
    /// op then cuts only what the edit names. Later edits add or remove.
    /// `Reset` brings the default back. `Replace` writes what it holds.
    ///
    /// Two keys name one item when [`MatchKey::matches`] holds: a pick and
    /// an exclude are stored keys, so the source is compared too.
    pub fn apply(
        &mut self,
        edit: SelectionEdit,
        op_default: &Selection,
    ) -> Result<(), SelectionRefusal> {
        match edit {
            SelectionEdit::Replace(selection) => *self = *selection,
            SelectionEdit::AddRule(rule) if self.is_default_of(op_default) => {
                *self = Self::from_rules(vec![rule]);
            }
            SelectionEdit::AddRule(rule) => {
                if !self.rules.contains(&rule) {
                    self.rules.push(rule);
                }
            }
            SelectionEdit::RemoveRule(index) => {
                if index >= self.rules.len() {
                    return Err(SelectionRefusal::new(
                        None,
                        RefusalCause::NoSuchRule {
                            index,
                            rules: self.rules.len(),
                        },
                    ));
                }
                self.rules.remove(index);
            }
            SelectionEdit::AddPick(key) if self.is_default_of(op_default) => {
                *self = Self::from_picks(vec![key]);
            }
            SelectionEdit::AddPick(key) => self.add_pick(key),
            SelectionEdit::RemovePick(key) => self.remove_pick(key),
            SelectionEdit::AddPicks(keys) | SelectionEdit::RemovePicks(keys) if keys.is_empty() => {
                return Err(SelectionRefusal::new(None, RefusalCause::NoKey));
            }
            SelectionEdit::AddPicks(keys) if self.is_default_of(op_default) => {
                // O-PICK, once per batch: the keys replace the default.
                let mut picks: Vec<MatchKey> = Vec::with_capacity(keys.len());
                for key in keys {
                    if !picks.iter().any(|k| k.matches(&key)) {
                        picks.push(key);
                    }
                }
                *self = Self::from_picks(picks);
            }
            SelectionEdit::AddPicks(keys) => {
                for key in keys {
                    self.add_pick(key);
                }
            }
            SelectionEdit::RemovePicks(keys) => {
                for key in keys {
                    self.remove_pick(key);
                }
            }
            SelectionEdit::Reset => *self = op_default.clone(),
        }
        Ok(())
    }

    /// Add one pick: drop every exclude that names the item, and store the
    /// key unless a pick names the item already. `AddPick` and `AddPicks`
    /// share this one body.
    fn add_pick(&mut self, key: MatchKey) {
        self.excludes.retain(|k| !k.matches(&key));
        if !self.picks.iter().any(|k| k.matches(&key)) {
            self.picks.push(key);
        }
    }

    /// Remove the picks that name the item; when none does, exclude it.
    /// `RemovePick` and `RemovePicks` share this one body.
    fn remove_pick(&mut self, key: MatchKey) {
        let before = self.picks.len();
        self.picks.retain(|k| !k.matches(&key));
        let removed = self.picks.len() != before;
        if !removed && !self.excludes.iter().any(|k| k.matches(&key)) {
            self.excludes.push(key);
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    fn key(x: f64) -> MatchKey {
        MatchKey {
            source: SourceRef::ToolpathModel,
            kind: ItemKind::Circle,
            layer: Some("H".into()),
            anchor: [x, 0.0],
            measure: 5.0,
            brep: None,
            z: None,
        }
    }

    #[test]
    fn a_layer_rule_is_a_kind_rule_with_one_layer() {
        let range = DiameterRange {
            nominal_mm: 5.0,
            tolerance_mm: 0.1,
        };
        for kinds in [
            vec![ItemKind::Loop],
            vec![ItemKind::Loop, ItemKind::Circle],
            vec![ItemKind::Circle],
            vec![],
        ] {
            for circle in [None, Some(range)] {
                assert_eq!(
                    Rule::layer_rules(SourceRef::ToolpathModel, "H", &kinds, circle),
                    Rule::kind_rules(SourceRef::ToolpathModel, &["H".to_owned()], &kinds, circle),
                    "{kinds:?} {circle:?}"
                );
            }
        }
        // Every layer: an empty layer list.
        let all = Rule::kind_rules(SourceRef::ToolpathModel, &[], &[ItemKind::Point], None);
        assert_eq!(all.len(), 1);
        assert!(all[0].filter.layers.is_empty());
    }

    #[test]
    fn a_batch_pick_replaces_the_default_once_o_pick() {
        let default = Selection::drawing();
        let mut s = Selection::drawing();
        s.apply(
            SelectionEdit::AddPicks(vec![key(1.0), key(2.0), key(1.0)]),
            &default,
        )
        .unwrap();
        assert_eq!(s, Selection::from_picks(vec![key(1.0), key(2.0)]));
        s.apply(SelectionEdit::AddPicks(vec![key(3.0), key(2.0)]), &default)
            .unwrap();
        assert_eq!(s.picks(), &[key(1.0), key(2.0), key(3.0)]);
        s.apply(
            SelectionEdit::RemovePicks(vec![key(1.0), key(3.0)]),
            &default,
        )
        .unwrap();
        assert_eq!(s.picks(), &[key(2.0)]);
        for empty in [
            SelectionEdit::AddPicks(Vec::new()),
            SelectionEdit::RemovePicks(Vec::new()),
        ] {
            assert_eq!(
                s.apply(empty, &default).unwrap_err().cause,
                RefusalCause::NoKey
            );
        }
    }

    #[test]
    fn the_three_defaults_are_told_apart() {
        assert!(Selection::drawing().is_drawing());
        assert!(!Selection::drawing().is_empty());
        assert!(Selection::empty().is_empty());
        assert!(!Selection::empty().is_drawing());
        assert!(Selection::stock().is_stock());
        assert!(!Selection::stock().is_drawing());
        assert!(!Selection::from_picks(vec![key(1.0)]).is_empty());
    }

    #[test]
    fn the_edits_keep_picks_and_excludes_apart() {
        let default = Selection::drawing();
        let mut s = Selection::drawing();
        // A remove of an item no pick names excludes it from the rules.
        s.apply(SelectionEdit::RemovePick(key(1.0)), &default)
            .unwrap();
        assert_eq!(s.excludes(), &[key(1.0)]);
        assert!(s.picks().is_empty());
        // An add of the same item removes the exclude and picks it.
        s.apply(SelectionEdit::AddPick(key(1.0)), &default).unwrap();
        assert!(s.excludes().is_empty());
        assert_eq!(s.picks(), &[key(1.0)]);
        // A second add does not pick it twice.
        s.apply(SelectionEdit::AddPick(key(1.0)), &default).unwrap();
        assert_eq!(s.picks().len(), 1);
        // A remove of a picked item removes the pick only.
        s.apply(SelectionEdit::RemovePick(key(1.0)), &default)
            .unwrap();
        assert!(s.picks().is_empty() && s.excludes().is_empty());
        // Rules: add once, remove by index, refuse a missing index.
        // Operator ruling O-PICK: the selection is the default again here,
        // so the first rule replaces the default rule.
        assert!(s.is_default_of(&default));
        let rule = Rule {
            source: SourceRef::ToolpathModel,
            filter: Filter {
                layers: vec!["H".into()],
                kinds: KindFilter::Only(vec![ItemKind::Circle]),
                ..Filter::default()
            },
        };
        s.apply(SelectionEdit::AddRule(rule.clone()), &default)
            .unwrap();
        assert_eq!(s.rules(), std::slice::from_ref(&rule));
        s.apply(SelectionEdit::AddRule(rule), &default).unwrap();
        assert_eq!(s.rules().len(), 1);
        s.apply(
            SelectionEdit::AddRule(Selection::drawing().rules()[0].clone()),
            &default,
        )
        .unwrap();
        assert_eq!(s.rules().len(), 2);
        s.apply(SelectionEdit::RemoveRule(0), &default).unwrap();
        assert_eq!(s.rules().len(), 1);
        let refusal = s.apply(SelectionEdit::RemoveRule(5), &default).unwrap_err();
        assert_eq!(
            refusal.cause,
            RefusalCause::NoSuchRule { index: 5, rules: 1 }
        );
        // Reset reads the op default the caller gives.
        s.apply(SelectionEdit::Reset, &Selection::empty()).unwrap();
        assert!(s.is_empty());
    }

    /// Operator ruling O-PICK: on a selection that is still the op
    /// default, the first pick (or rule) replaces the default. Later picks
    /// add; `Reset` brings the default back. An empty default is never
    /// replaced: a pick on it adds.
    #[test]
    fn a_pick_replaces_the_default_rule_o_pick() {
        let default = Selection::drawing();
        let mut s = Selection::drawing();
        s.apply(SelectionEdit::AddPick(key(1.0)), &default).unwrap();
        assert_eq!(s, Selection::from_picks(vec![key(1.0)]));
        s.apply(SelectionEdit::AddPick(key(2.0)), &default).unwrap();
        assert_eq!(s, Selection::from_picks(vec![key(1.0), key(2.0)]));
        s.apply(SelectionEdit::Reset, &default).unwrap();
        assert!(s.is_default_of(&default));
        // A rule on the default replaces it too.
        let rule = Rule {
            source: SourceRef::ToolpathModel,
            filter: Filter {
                layers: vec!["H".into()],
                ..Filter::default()
            },
        };
        s.apply(SelectionEdit::AddRule(rule.clone()), &default)
            .unwrap();
        assert_eq!(s, Selection::from_rules(vec![rule]));
        // The Face default (the stock) takes the same rule.
        let mut face = Selection::stock();
        face.apply(SelectionEdit::AddPick(key(1.0)), &Selection::stock())
            .unwrap();
        assert_eq!(face, Selection::from_picks(vec![key(1.0)]));
        // An empty default: the pick adds.
        let mut pins = Selection::empty();
        assert!(!pins.is_default_of(&Selection::empty()));
        pins.apply(SelectionEdit::AddPick(key(1.0)), &Selection::empty())
            .unwrap();
        assert_eq!(pins, Selection::from_picks(vec![key(1.0)]));
    }

    #[test]
    fn only_the_own_model_is_stored_as_the_toolpath_model() {
        let own = ModelId::from_raw(3);
        let on = |source| MatchKey { source, ..key(1.0) };
        let edit = SelectionEdit::AddPick(on(SourceRef::Model(own)));
        assert_eq!(
            edit.for_toolpath_model(own),
            SelectionEdit::AddPick(on(SourceRef::ToolpathModel))
        );
        // Another model and the stock stay as they are.
        for source in [SourceRef::Model(ModelId::from_raw(4)), SourceRef::Stock] {
            let edit = SelectionEdit::RemovePick(on(source));
            assert_eq!(edit.clone().for_toolpath_model(own), edit);
        }
        // A whole selection: rules, picks and excludes.
        let rule = Rule {
            source: SourceRef::Model(own),
            filter: Filter::default(),
        };
        let s = Selection::from_parts(
            vec![rule],
            vec![on(SourceRef::Model(own))],
            vec![on(SourceRef::Model(own))],
        )
        .for_toolpath_model(own);
        assert!(
            s.sources().all(|src| src == SourceRef::ToolpathModel),
            "{s:?}"
        );
    }

    #[test]
    fn the_wire_is_snake_case_and_skips_empty_lists() {
        let mut s = Selection::empty();
        s.apply(
            SelectionEdit::AddRule(Rule {
                source: SourceRef::ToolpathModel,
                filter: Filter {
                    layers: vec!["CUT".into()],
                    kinds: KindFilter::Only(vec![ItemKind::Loop, ItemKind::Circle]),
                    ..Filter::default()
                },
            }),
            &Selection::empty(),
        )
        .unwrap();
        s.apply(SelectionEdit::AddPick(key(12.0)), &Selection::empty())
            .unwrap();
        let text = toml::to_string(&s).unwrap();
        assert!(text.contains("source = \"toolpath_model\""), "{text}");
        assert!(text.contains("only = [\"loop\", \"circle\"]"), "{text}");
        assert!(text.contains("kind = \"circle\""), "{text}");
        assert!(!text.contains("excludes"), "{text}");
        assert!(!text.contains("brep"), "{text}");
        let back: Selection = toml::from_str(&text).unwrap();
        assert_eq!(back, s);
        // The default kinds filter is `served`.
        let drawing = toml::to_string(&Selection::drawing()).unwrap();
        assert!(drawing.contains("kinds = \"served\""), "{drawing}");
        assert_eq!(
            toml::from_str::<Selection>(&drawing).unwrap(),
            Selection::drawing()
        );
        // An unknown key refuses.
        assert!(toml::from_str::<Selection>("pickz = []").is_err());
    }

    /// Geometry selection P4.5: a layer pick with loops and Ø-limited
    /// circles gives TWO rules, because a filter with a diameter admits
    /// circles only. One kind gives one rule; no kind gives none.
    #[test]
    fn a_layer_pick_splits_the_circle_range_into_its_own_rule_p4_5() {
        let range = DiameterRange {
            nominal_mm: 5.0,
            tolerance_mm: 0.1,
        };
        let rules = Rule::layer_rules(
            SourceRef::ToolpathModel,
            "CUT",
            &[ItemKind::Loop, ItemKind::Circle],
            Some(range),
        );
        assert_eq!(rules.len(), 2, "{rules:?}");
        assert_eq!(
            rules[0].filter.kinds,
            KindFilter::Only(vec![ItemKind::Loop])
        );
        assert_eq!(rules[0].filter.diameter, None);
        assert_eq!(
            rules[1].filter.kinds,
            KindFilter::Only(vec![ItemKind::Circle])
        );
        assert_eq!(rules[1].filter.diameter, Some(range));
        assert!(
            rules
                .iter()
                .all(|r| r.filter.layers == vec!["CUT".to_owned()])
        );
        // No circle chip: the range is not used.
        let one = Rule::layer_rules(
            SourceRef::ToolpathModel,
            "CUT",
            &[ItemKind::Loop],
            Some(range),
        );
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].filter.diameter, None);
        assert!(Rule::layer_rules(SourceRef::ToolpathModel, "CUT", &[], None).is_empty());
    }

    /// Geometry selection P4.3/P4.5: under the default the first rules
    /// replace it; after it one rule adds and two rules append in one edit.
    #[test]
    fn added_rules_replace_the_default_only_p4_5() {
        let own = ModelId::from_raw(0);
        let rule = |layer: &str| Rule {
            source: SourceRef::ToolpathModel,
            filter: Filter {
                layers: vec![layer.to_owned()],
                ..Filter::default()
            },
        };
        let default = Selection::drawing();
        assert_eq!(
            SelectionEdit::add_rules(&default, &default, own, vec![rule("A")]),
            Some(SelectionEdit::Replace(Box::new(Selection::from_rules(
                vec![rule("A")]
            ))))
        );
        let custom = Selection::from_rules(vec![rule("A")]);
        assert_eq!(
            SelectionEdit::add_rules(&custom, &default, own, vec![rule("B")]),
            Some(SelectionEdit::AddRule(rule("B")))
        );
        let Some(SelectionEdit::Replace(both)) =
            SelectionEdit::add_rules(&custom, &default, own, vec![rule("A"), rule("C")])
        else {
            panic!("two rules after the default give one Replace");
        };
        assert_eq!(both.rules(), &[rule("A"), rule("C")]);
        // An empty default (a boundary with no item) is not replaced.
        assert_eq!(
            SelectionEdit::add_rules(
                &Selection::empty(),
                &Selection::empty(),
                own,
                vec![rule("A")]
            ),
            Some(SelectionEdit::AddRule(rule("A")))
        );
        assert_eq!(
            SelectionEdit::add_rules(&custom, &default, own, vec![]),
            None
        );
    }
}
