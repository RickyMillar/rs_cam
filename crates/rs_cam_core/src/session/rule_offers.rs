//! The "Rule…" offers of one picked item (geometry selection GT-3, rule
//! L-R4 of the Geometry tab design).
//!
//! An offer is one `Filter` from the facts of ONE item that the operator
//! picked: its kind, its diameter, its plane Z, its layer. Nothing offers
//! an item that the operator did not pick, so the offers are not
//! recognition ("auto magic is a wizard", operator ruling 2026-10-22).
//!
//! Each offer carries its match count and the ONE edit that it writes
//! (`SelectionEdit::Replace`, one undo step): the new rule replaces the
//! picks that it holds and every rule whose members it holds. An offer that
//! an existing rule already holds writes nothing (`edit` is `None`).

use crate::selection::{
    CAD_ROUNDING_MM, DiameterRange, FaceFilter, Facing, Filter, ItemKind, KindFilter, MatchKey,
    Rule, Selection, SelectionEdit, Shape, SourceRef, ZRange, kind_noun,
};
use crate::session::{ProjectSession, SessionError, ToolpathConfig};

use super::SetupEvalContext;
use super::selection_status::{FaceAccess, SelectionTarget, with_selection};

/// One rule that the Geometry tab offers for a picked item.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleOffer {
    /// The text of the offer: "All circles Ø 6.00 on this model".
    pub label: String,
    /// The rule (source in the toolpath form).
    pub rule: Rule,
    /// The count of the items that the rule matches.
    pub count: usize,
    /// The edit that the offer writes, or `None` when a rule of the
    /// selection already holds every item of the offer.
    pub edit: Option<SelectionEdit>,
}

/// The half width of the Z range of a Z offer (mm): the rounding of a CAD
/// length ([`CAD_ROUNDING_MM`]), so two planes that a CAD export writes
/// as one Z are one offer.
const Z_HALF_RANGE_MM: f64 = CAD_ROUNDING_MM;

impl ProjectSession {
    /// The "Rule…" offers of the item `key` in the selection `target` of
    /// `tc` (GT-3, L-R4). `key` names an item of a model (the key of a
    /// selected list row). The facts are read in the setup frame, as a
    /// rule filter reads them (`Filter::z`, `FaceFilter::z`).
    ///
    /// - A circle: the circles of its diameter, then every circle.
    /// - An up face: the up faces at its plane Z. Every face: every up face
    ///   of the model. A down face on a boundary: the down faces at its
    ///   plane Z (SEL-7).
    /// - A model ring, edge chain or vertex (a Z item): the items of its
    ///   kind at its Z, then every item of its kind.
    /// - A drawing loop, path or point: the items of its kind on its layer
    ///   (when it has one), then every item of its kind.
    ///
    /// # Errors
    ///
    /// [`SessionError`] when the selection does not resolve.
    pub fn rule_offers(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: &MatchKey,
    ) -> Result<Vec<RuleOffer>, SessionError> {
        let Some(current) = tc.selection_of(target)? else {
            return Ok(Vec::new());
        };
        let model = tc.model_id;
        let current = current.clone().for_toolpath_model(model);
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        let source = match key.source {
            SourceRef::Model(id) if id == model => SourceRef::ToolpathModel,
            other => other,
        };
        // The facts of the item, in the setup frame.
        let frames = self
            .resolve_spec_frames(
                tc,
                &Selection::from_picks(vec![key.clone()]),
                &ctx,
                FaceAccess::Peek,
                super::selection_status::target_uses(tc, SelectionTarget::Op),
            )
            .map_err(|e| SessionError::OperationFailed(e.to_string()))?;
        let Some(item) = frames.setup.items().first() else {
            return Ok(Vec::new());
        };
        let item = item.item();
        let only = |kind: ItemKind| Filter {
            kinds: KindFilter::Only(vec![kind]),
            ..Filter::default()
        };
        let z_range = |z: f64| ZRange {
            min: z - Z_HALF_RANGE_MM,
            max: z + Z_HALF_RANGE_MM,
        };
        let kind = item.kind();
        let plural = kind_noun(kind, 2);
        let mut filters: Vec<(String, Filter)> = Vec::new();
        match item.shape() {
            Shape::Circle { diameter, .. } => {
                filters.push((
                    format!("All {plural} \u{00D8} {diameter:.2} on this model"),
                    Filter {
                        diameter: Some(DiameterRange {
                            nominal_mm: *diameter,
                            tolerance_mm: CAD_ROUNDING_MM,
                        }),
                        ..only(ItemKind::Circle)
                    },
                ));
                filters.push((format!("All {plural} on this model"), only(kind)));
            }
            Shape::Face(face) => {
                if Facing::Up.admits(face.normal.z) {
                    filters.push((
                        format!("All up {plural} at Z {:.2}", face.plane_z),
                        Filter {
                            face: Some(FaceFilter {
                                facing: Facing::Up,
                                z: Some(z_range(face.plane_z)),
                            }),
                            ..only(ItemKind::Face)
                        },
                    ));
                }
                // SEL-7: every up face of the model.
                filters.push((
                    format!("All up {plural} on this model"),
                    Filter {
                        face: Some(FaceFilter {
                            facing: Facing::Up,
                            z: None,
                        }),
                        ..only(ItemKind::Face)
                    },
                ));
                // SEL-7: the down faces at the plane Z, for a boundary only
                // (`view` refuses a down face in an op area, `FaceFacesDown`).
                if target == SelectionTarget::Boundary && Facing::Down.admits(face.normal.z) {
                    filters.push((
                        format!("All down {plural} at Z {:.2}", face.plane_z),
                        Filter {
                            face: Some(FaceFilter {
                                facing: Facing::Down,
                                z: Some(z_range(face.plane_z)),
                            }),
                            ..only(ItemKind::Face)
                        },
                    ));
                }
            }
            Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) => {
                match (item.z(), item.layer()) {
                    (Some(z), _) => filters.push((
                        format!("All {plural} at Z {z:.2}"),
                        Filter {
                            z: Some(z_range(z)),
                            ..only(kind)
                        },
                    )),
                    (None, Some(layer)) => filters.push((
                        format!("All {plural} on layer {layer}"),
                        Filter {
                            layers: vec![layer.to_owned()],
                            ..only(kind)
                        },
                    )),
                    (None, None) => {}
                }
                filters.push((format!("All {plural} on this model"), only(kind)));
            }
        }
        // The members of each rule that the selection holds now.
        let mut held: Vec<Vec<MatchKey>> = Vec::with_capacity(current.rules().len());
        for rule in current.rules() {
            held.push(self.rule_members(tc, target, &ctx, rule, current.excludes())?);
        }
        let mut offers = Vec::with_capacity(filters.len());
        for (label, filter) in filters {
            let rule = Rule { source, filter };
            let matches: Vec<MatchKey> = self
                .build_selection_report(
                    &with_selection(tc, target, Selection::from_rules(vec![rule.clone()])),
                    target,
                    &ctx,
                )?
                .selected
                .iter()
                .map(|k| k.clone().for_toolpath_model(model))
                .collect();
            let holds =
                |members: &[MatchKey], key: &MatchKey| members.iter().any(|m| m.matches(key));
            let already = !matches.is_empty()
                && held
                    .iter()
                    .any(|members| matches.iter().all(|k| holds(members, k)));
            let edit = (!already).then(|| {
                let mut rules: Vec<Rule> = current
                    .rules()
                    .iter()
                    .zip(&held)
                    .filter(|(_, members)| {
                        members.is_empty() || !members.iter().all(|m| holds(&matches, m))
                    })
                    .map(|(rule, _)| rule.clone())
                    .collect();
                rules.push(rule.clone());
                let picks = current
                    .picks()
                    .iter()
                    .filter(|p| !holds(&matches, p))
                    .cloned()
                    .collect();
                SelectionEdit::Replace(Box::new(Selection::from_parts(
                    rules,
                    picks,
                    current.excludes().to_vec(),
                )))
            });
            offers.push(RuleOffer {
                label,
                rule,
                count: matches.len(),
                edit,
            });
        }
        Ok(offers)
    }
}
