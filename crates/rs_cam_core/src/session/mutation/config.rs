//! Project-wide configuration edits — dressups, heights, boundary, rest
//! analysis, stock, post, machine and tools — and their invalidation.
//!
//! Split out of `session/mutation.rs` by P4. Every method here is an
//! inherent method on [`ProjectSession`], so its path does not change.

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, BoundarySource, DressupConfig, HeightsConfig};
use crate::compute::stock_config::{ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::geo::BoundingBox3;
use crate::session::{Effects, ProjectSession, SessionError, SimulationDropCause};

use super::post_change_reaches_motion;

impl ProjectSession {
    /// Replace the dressup config for a toolpath, invalidating its cached result.
    #[instrument(skip(self, dressups))]
    pub(crate) fn set_dressup_config(
        &mut self,
        index: usize,
        mut dressups: DressupConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            // Enforce the per-operation dressup invariant so incompatible
            // combinations can't be introduced via this API.
            dressups.normalize_for_op(tc.operation.op_type());
            tc.dressups = dressups;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Update a single dressup field by merging a JSON patch onto the existing
    /// [`DressupConfig`]. Only the specified field is changed. Invalidates
    /// the cached result.
    #[instrument(skip(self, value))]
    pub(crate) fn set_dressup_field(
        &mut self,
        index: usize,
        key: &str,
        value: serde_json::Value,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            session.set_dressup_field_impl(index, key, value)
        })
    }

    /// The body of [`Self::set_dressup_field`].
    ///
    /// The setter wraps this in the [`Effects`] combinator. The body
    /// stays a separate function because the JSON merge is long and the
    /// wrapper reads better with one call in it.
    fn set_dressup_field_impl(
        &mut self,
        index: usize,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let mut merged = serde_json::to_value(&tc.dressups)
            .map_err(|e| SessionError::InvalidParam(format!("dressup serialize: {e}")))?;
        match merged.as_object_mut() {
            Some(obj) => {
                // The field table decides what a legal key is. The serialized
                // instance cannot: five `Option` fields carry
                // `skip_serializing_if = "Option::is_none"`, so on a fresh
                // config `lead_in_feed_rate` is absent from the object and
                // a membership test on the object refused it (wave 3,
                // 2026-09-18). The coercion arms below still read the
                // instance; an absent optional takes the value as sent.
                if !DressupConfig::FIELD_DEFS.iter().any(|d| d.name == key) {
                    return Err(SessionError::InvalidParam(format!(
                        "unknown dressup field '{key}'"
                    )));
                }
                // CUT-13: a value field now lives inside its dressup's
                // `Option`. Patching one while the dressup is off used to
                // write a number nothing read; it would now be dropped by
                // the round trip instead. Refuse and name the enable key,
                // so the caller learns the order rather than losing a set.
                if let Some(owner) = DressupConfig::owner_of_value_field(key)
                    && !tc.dressups.is_dressup_enabled(owner)
                {
                    return Err(SessionError::InvalidParam(format!(
                        "dressup field '{key}' belongs to '{owner}', which is off; \
                         enable '{owner}' first"
                    )));
                }
                // Roadmap E.6.b — symmetric coercion mirroring
                // `set_toolpath_param`'s wildcard: 0/1 -> bool when the
                // existing field is a boolean, and numeric-string ->
                // number when the existing field is numeric. Lets MCP
                // clients that always serialize numerically (or always
                // stringify) drive boolean and numeric dressup fields.
                let value = match (obj.get(key), &value) {
                    (Some(existing), serde_json::Value::Number(n)) if existing.is_boolean() => {
                        match n.as_i64() {
                            Some(0) => serde_json::Value::Bool(false),
                            Some(1) => serde_json::Value::Bool(true),
                            _ => value,
                        }
                    }
                    // Some MCP wrappers double-encode strings ("0" arrives
                    // as the literal 3-char string `"0"`). Strip surrounding
                    // quotes before parsing so both forms work for booleans
                    // and numerics.
                    (Some(existing), serde_json::Value::String(s)) if existing.is_boolean() => {
                        match crate::session::compute::strip_outer_quotes(s)
                            .to_ascii_lowercase()
                            .as_str()
                        {
                            "0" | "false" => serde_json::Value::Bool(false),
                            "1" | "true" => serde_json::Value::Bool(true),
                            _ => value,
                        }
                    }
                    (Some(existing), serde_json::Value::String(s)) if existing.is_number() => {
                        match crate::session::compute::strip_outer_quotes(s).parse::<f64>() {
                            Ok(n) => serde_json::Number::from_f64(n)
                                .map(serde_json::Value::Number)
                                .unwrap_or(value),
                            Err(_) => value,
                        }
                    }
                    _ => value,
                };
                obj.insert(key.to_owned(), value);
            }
            None => {
                return Err(SessionError::InvalidParam(
                    "dressup config is not an object".to_owned(),
                ));
            }
        }
        let mut new_cfg: DressupConfig = serde_json::from_value(merged)
            .map_err(|e| SessionError::InvalidParam(format!("dressup patch: {e}")))?;
        // Enforce the per-operation dressup invariant on every patch.
        new_cfg.normalize_for_op(tc.operation.op_type());
        tc.dressups = new_cfg;
        let enabled = tc.enabled;
        self.invalidate_result_chain(index, enabled);
        Ok(())
    }

    /// Set the stock_source for a toolpath (Fresh vs FromRemainingStock).
    /// Invalidates the cached result.
    #[instrument(skip(self))]
    pub(crate) fn set_stock_source(
        &mut self,
        index: usize,
        source: crate::compute::config::StockSource,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.stock_source = source;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Replace the heights config for a toolpath, invalidating its cached result.
    #[instrument(skip(self, heights))]
    pub(crate) fn set_heights_config(
        &mut self,
        index: usize,
        heights: HeightsConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            // P3.2: a pinned Bottom Z on an `OpFloor` op refuses; nothing
            // is written.
            let before = std::mem::replace(&mut tc.heights, heights);
            if let Err(refusal) = tc.floor_conflict() {
                tc.heights = before;
                return Err(refusal);
            }
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Replace the boundary config for a toolpath, invalidating its cached
    /// result. Also runs the demand-driven rest-analysis producer hook (see
    /// [`Self::auto_enable_rest_analysis_for_source`]): wiring this toolpath
    /// to an enabled `DerivedRestRegions` source means that source must
    /// actually produce rest regions.
    #[instrument(skip(self, boundary))]
    pub(crate) fn set_boundary_config(
        &mut self,
        index: usize,
        boundary: BoundaryConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            let auto_enable_source = match (boundary.enabled, &boundary.source) {
                (true, BoundarySource::DerivedRestRegions { source_toolpath_id }) => {
                    Some(*source_toolpath_id)
                }
                _ => None,
            };
            tc.boundary = boundary;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            if let Some(source_id) = auto_enable_source {
                // The producer hook reports its own effects. This
                // wrapper's snapshot spans both mutations, so the outer
                // answer already carries what the hook dropped.
                let _ = session.auto_enable_rest_analysis_for_source(source_id);
            }
            Ok(())
        })
    }

    /// Demand-driven rest-analysis producer hook (P2 pencil-panel
    /// consolidation): call when a toolpath's boundary becomes an *enabled*
    /// `BoundarySource::DerivedRestRegions { source_toolpath_id }` — the
    /// newly-wired consumer needs `source_toolpath_id`'s toolpath to actually
    /// produce rest regions. Flips that toolpath's `rest_analysis.enabled` on
    /// and invalidates its cached result so the next generation attaches
    /// `rest_grid` / `rest_regions` via
    /// `compute::execute::attach_generic_rest_analysis`.
    ///
    /// A no-op (returns `None`) when the source toolpath doesn't exist,
    /// already has rest analysis enabled, or is itself a `rest_depth`
    /// pencil — that detector attaches the same artifacts on its own, and
    /// `attach_generic_rest_analysis` already skips itself once they're
    /// present, so forcing the flag there would just be a redundant,
    /// confusing UI toggle (the GUI hides it entirely for these ops).
    ///
    /// `None` says the call changed nothing. It is not an error, and it
    /// is not an empty [`Effects`].
    #[instrument(skip(self))]
    pub(crate) fn auto_enable_rest_analysis_for_source(
        &mut self,
        source_id: crate::ids::ToolpathId,
    ) -> Option<Effects> {
        let (idx, tc) = self.find_toolpath_config_by_id(source_id)?;
        if tc.rest_analysis.enabled {
            return None;
        }
        if let OperationConfig::Pencil(cfg) = &tc.operation
            && cfg.detector == crate::finish::pencil::PencilDetector::RestDepth
        {
            return None;
        }
        Some(self.with_effects(Some(idx), move |session| {
            if let Some((_, tc)) = session.find_toolpath_config_by_id_mut(source_id) {
                tc.rest_analysis.enabled = true;
            }
            session.drop_result(idx);
            tracing::info!(
                source_toolpath_id = source_id.0,
                "Auto-enabled rest analysis: a toolpath now consumes this one's \
                 derived rest regions as a machining boundary"
            );
        }))
    }

    /// Replace the rest-analysis config (P2.5) for a toolpath, invalidating
    /// its cached result. Mirrors `set_boundary_config`.
    #[instrument(skip(self, rest_analysis))]
    pub(crate) fn set_rest_analysis_config(
        &mut self,
        index: usize,
        rest_analysis: crate::compute::config::RestAnalysisConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.rest_analysis = rest_analysis;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    // ── Invalidation helpers ─────────────────────────────────────
    //
    // For immediate-mode UI panels that mutate session fields in-place
    // (via `stock_mut()` / `machine_mut()` / `tools_mut()`), these methods
    // ensure the cache is properly cleared after the edit completes.

    /// Invalidate after the stock config was mutated in-place.
    ///
    /// Drops EVERY toolpath result, not only the simulation — see
    /// [`Self::drop_all_results`] for the operator ruling behind that.
    #[instrument(skip(self))]
    pub(crate) fn invalidate_stock(&mut self) -> Effects {
        self.with_effects(None, |session| session.drop_all_results())
    }

    /// Invalidate cached simulation after machine profile was mutated in-place.
    #[instrument(skip(self))]
    pub(crate) fn invalidate_machine(&mut self) -> Effects {
        self.with_effects(None, |session| {
            session.drop_simulation(SimulationDropCause::Machine);
        })
    }

    /// Invalidate cached results for all toolpaths that reference a given
    /// tool. [`Effects::stale`] carries the toolpath indices whose result
    /// was dropped, so a caller can request their regeneration.
    #[instrument(skip(self))]
    pub(crate) fn invalidate_tool(&mut self, tool_id: usize) -> Effects {
        self.with_effects(None, move |session| session.drop_tool_results(tool_id))
    }

    /// Drop the cached result of every toolpath that depends on one
    /// tool, plus the simulation.
    ///
    /// The raw half of [`Self::invalidate_tool`].
    /// [`Self::set_tool_param`] calls it directly, so the two routes
    /// cannot drop different sets and neither nests one [`Effects`]
    /// construction inside another.
    ///
    /// SES-07: the direct set is the SEED, not the answer. A re-dialled
    /// cutter leaves other stock, so a same-setup
    /// `StockSource::FromRemainingStock` op on ANOTHER tool, and a
    /// `BoundarySource::DerivedRestRegions` consumer, go stale with it.
    /// The walk to fixpoint lives in
    /// [`Self::drop_results_and_their_dependents`], which every other wide
    /// mutation path shares. This door ran a flat `drop_result` loop
    /// before, and kept those downstream results.
    pub(crate) fn drop_tool_results(&mut self, tool_id: usize) {
        // A toolpath depends on a tool through TWO doors, not one. The
        // obvious door is `tool_id` — the cutter that machines it. The
        // second is a `PlannedTierRegions` boundary, whose islands are
        // "the coarsest tool on THIS LADDER that holds each cell":
        // re-dial any ladder member and the fine tier's territory moves,
        // even though the fine tier's own cutter is untouched. Missing
        // that door leaves a generated tier bounded by a map that no
        // longer exists.
        let stale: Vec<usize> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(_, tc)| {
                tc.tool_id == tool_id
                    || matches!(
                        &tc.boundary.source,
                        BoundarySource::PlannedTierRegions { tool_ids, .. }
                            if tc.boundary.enabled && tool_ids.contains(&tool_id)
                    )
            })
            .map(|(idx, _)| idx)
            .collect();
        self.drop_results_and_their_dependents(&stale);
    }

    /// Invalidate cached results for every toolpath that machines a given
    /// model, plus the simulation.
    ///
    /// A model reload or rescale replaces the geometry every one of those
    /// results was generated against, so none of them still describes the
    /// project. [`Effects::stale`] carries the toolpath indices whose
    /// result was dropped, so a caller can request their regeneration.
    #[instrument(skip(self))]
    pub(crate) fn invalidate_model(&mut self, model_id: usize) -> Effects {
        self.with_effects(None, move |session| {
            session.drop_results_for_model(model_id);
        })
    }

    /// Drop the cached result of every toolpath bound to one model, and
    /// the simulation.
    ///
    /// The rule [`Self::invalidate_model`] and
    /// [`Self::adopt_model_geometry`] share. The second one writes the
    /// geometry and runs this in ONE mutation, so the two halves cannot
    /// drift apart the way the three GUI refresh doors did
    /// (G-RELOADTARGETS, G-RESCALESTALE).
    ///
    /// SES-07: a downstream `StockSource::FromRemainingStock` result was
    /// generated against the stock the OLD geometry left, so it is stale
    /// too. This door seeds the same chain walk, through
    /// [`Self::drop_results_and_their_dependents`].
    ///
    /// A toolpath reads a model through every door that
    /// [`crate::session::ToolpathConfig::models_read`] lists: its own model, its op
    /// selection, the Project Curve surface and an enabled boundary
    /// selection (geometry selection P2.5). A reload of any of them moves
    /// the cut, so the toolpath drops. S2 adds a setup stock change whose
    /// geometry names the model. K1 adds a setup obstacle that names it.
    pub(super) fn drop_results_for_model(&mut self, model_id: usize) {
        let affected: Vec<usize> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(_, tc)| tc.models_read().contains(&model_id))
            .map(|(idx, _)| idx)
            .collect();
        self.drop_results_and_their_dependents(&affected);
        // S2: an enabled stock change that reads the model changes the stock
        // in a new way. The first such setup and every later one are stale.
        let first_reader = self.setups.iter().position(|setup| {
            setup.stock_changes.iter().any(|change| {
                change.enabled && change.geometry.reads_model(crate::ids::ModelId(model_id))
            })
        });
        if let Some(from_setup) = first_reader {
            self.drop_stock_change_dependents(from_setup);
        }
        // K1: an enabled obstacle that reads the model moves where the
        // tool may go in its setup.
        let readers: Vec<usize> = self
            .setups
            .iter()
            .enumerate()
            .filter(|(_, setup)| {
                setup
                    .obstacles
                    .iter()
                    .any(|o| o.enabled && o.geometry.reads_model(crate::ids::ModelId(model_id)))
            })
            .map(|(index, _)| index)
            .collect();
        for setup_index in readers {
            self.drop_obstacle_dependents(setup_index);
        }
    }

    /// Replace one model's geometry, and drop the results that read it.
    ///
    /// The door of the `AdoptModelGeometry` command row. The GUI holds
    /// three model-refresh doors — rescale, reload and relink — and each
    /// one re-imports the file itself. The import belongs to the surface
    /// that owns the file dialogue; core adopts the geometry that import
    /// produced and reports what the adoption dropped.
    ///
    /// `geometry` supplies the fields
    /// [`LoadedModel::adopt_geometry`](super::LoadedModel::adopt_geometry)
    /// names. The id and the name never move: keeping the id is the point
    /// of a refresh, because every `ToolpathConfig::model_id` goes on
    /// naming this model.
    ///
    /// `units` of `None` means the caller overrides no unit declaration,
    /// and the record keeps the units it carries. Only the rescale door
    /// sends `Some`: a rescale IS the declared-units change.
    ///
    /// The row refuses an id that names no model, rather than reporting
    /// an empty [`Effects`] a reader could take for "nothing to do".
    #[instrument(skip(self, geometry))]
    pub(crate) fn adopt_model_geometry(
        &mut self,
        model_id: usize,
        geometry: super::LoadedModel,
        units: Option<ModelUnits>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let Some(model) = session.models.iter_mut().find(|m| m.id == model_id) else {
                let known: Vec<String> = session.models.iter().map(|m| m.id.to_string()).collect();
                return Err(SessionError::MissingGeometry(format!(
                    "Model id {model_id} not found; project model ids: [{}]",
                    known.join(", ")
                )));
            };
            // P4.9: the units first. The ledger replay inside the adopt
            // converts the stored file-unit edits at the record's scale.
            if let Some(units) = units {
                model.units = Some(units);
            }
            model.adopt_geometry(geometry);
            // The id did NOT change, so no signature comparison can see
            // this. `generation_inputs_signature` carries `model_id`, and
            // the id is what a refresh keeps. Dropping by id is therefore
            // the only instrument that answers.
            session.drop_results_for_model(model_id);
            Ok(())
        })
    }

    /// Change the edit ledger of one drawing, replay it on the items of
    /// the import door, and drop the results that read the model.
    ///
    /// The door of the `EditModelDrawing` row (geometry selection P4.9).
    /// An `Add` edit is in model mm (keys from `open_shapes` or
    /// `list_model_items`); the door converts it once to the file units of
    /// the ledger. A `Replace` list is the stored ledger, in file units
    /// (the project-file JSON). The keys of a new edit take the ledger form
    /// ([`DrawingEdit::in_ledger_form`](crate::selection::DrawingEdit::in_ledger_form)),
    /// so the GUI and MCP store one edit for one act. The row refuses:
    ///
    /// - an id that names no model, and a model with no drawing item;
    /// - an `Add` whose edit does not apply to the items now (the reason
    ///   is the outcome text, so a cross-layer join refuses, D14);
    /// - a `Remove` of an index past the end.
    ///
    /// Each change stales the toolpaths that read the model, also when the
    /// items do not change: an edit is a geometry act.
    #[instrument(skip(self))]
    pub(crate) fn edit_model_drawing(
        &mut self,
        model_id: usize,
        edit: crate::session::LedgerEdit,
    ) -> Result<Effects, SessionError> {
        use crate::session::LedgerEdit;
        self.try_with_effects(None, move |session| {
            let Some(model) = session.models.iter_mut().find(|m| m.id == model_id) else {
                let known: Vec<String> = session.models.iter().map(|m| m.id.to_string()).collect();
                return Err(SessionError::MissingGeometry(format!(
                    "Model id {model_id} not found; project model ids: [{}]",
                    known.join(", ")
                )));
            };
            let door = model.door_items();
            if !door.has_drawing() {
                return Err(SessionError::InvalidParam(format!(
                    "Model {model_id} holds no drawing item; only a drawing (DXF, SVG) has open paths to edit"
                )));
            }
            let mut edits = model.drawing.edits().to_vec();
            let scale = model.drawing_scale();
            let added = match edit {
                // The new edit is in model mm; the ledger stores file units.
                LedgerEdit::Add(new) => {
                    edits.push(new.in_ledger_form().scaled(1.0 / scale));
                    true
                }
                LedgerEdit::Remove(index) => {
                    if index >= edits.len() {
                        return Err(SessionError::InvalidParam(format!(
                            "Model {model_id} has {} drawing edits; no edit has index {index}",
                            edits.len()
                        )));
                    }
                    edits.remove(index);
                    false
                }
                LedgerEdit::Replace(all) => {
                    edits = all
                        .into_iter()
                        .map(crate::selection::DrawingEdit::in_ledger_form)
                        .collect();
                    false
                }
            };
            if added {
                let in_mm: Vec<_> = edits.iter().cloned().map(|e| e.scaled(scale)).collect();
                let (_, outcomes) = crate::selection::apply_drawing_edits(&door, &in_mm);
                if let Some(Err(lost)) = outcomes.last().map(|o| &o.result) {
                    return Err(SessionError::InvalidParam(format!(
                        "The drawing edit does not apply: {}",
                        lost.label()
                    )));
                }
            }
            model.replay_drawing_edits(door, edits);
            session.drop_results_for_model(model_id);
            Ok(())
        })
    }

    // ── Global config ─────────────────────────────────────────────

    /// Replace the stock configuration.
    ///
    /// Drops every toolpath result — see [`Self::drop_all_results`].
    #[instrument(skip(self, stock))]
    pub(crate) fn set_stock_config(&mut self, stock: StockConfig) -> Effects {
        self.with_effects(None, move |session| {
            session.stock = stock;
            session.drop_all_results();
        })
    }

    /// Update stock dimensions from a bounding box (used by `auto_from_model`
    /// to size the stock around imported geometry). Invalidates simulation.
    ///
    /// For 2D polygon bboxes (zero Z extent), [`StockConfig::update_from_bbox`]
    /// preserves the existing Z so attaching an SVG/DXF doesn't collapse stock
    /// thickness — see F-13 in the April 2026 review.
    #[instrument(skip(self))]
    pub(crate) fn update_stock_from_bbox(&mut self, bbox: &BoundingBox3) -> Effects {
        self.with_effects(None, move |session| {
            session.stock.update_from_bbox(bbox);
            session.drop_all_results();
        })
    }

    /// Replace the post-processor configuration.
    ///
    /// The door of the `SetPostConfig` command row. It clears the
    /// simulation ONLY when `post_change_reaches_motion` says the edit
    /// moved a field that reaches emitted motion or the simulated clock.
    /// **This is a behaviour change (WP17).**
    ///
    /// The method wrote `simulation = None` on EVERY call. Both GUI save
    /// doors called it before every save, with the block the session
    /// already held, so a save dropped the simulation. `start` then
    /// refused every `FromRemainingStock` operation, because WP11b reads
    /// the rest snapshot from that field, and nothing re-adopted.
    ///
    /// The per-field rule:
    ///
    /// - `format` — read at emit time. No invalidation.
    /// - `spindle_strategy` — read at suggest time. No invalidation.
    /// - `spindle_speed` — the emitted S word, read at emit time, and
    ///   `SimulationRequest::spindle_rpm`. The cut trace's gate readings
    ///   carry the old value, so the simulation goes.
    /// - `high_feedrate_mode`, `high_feedrate` — the rate the simulated
    ///   clock runs rapids at (`SimulationRequest::rapid_feed_mm_min`).
    ///   The runtime and air-cut readings carry the old rate, so the
    ///   simulation goes.
    /// - `safe_z` — `SetupEvalContext` resolves it at GENERATION time,
    ///   and the generators emit rapids at that height. The simulation
    ///   goes.
    ///
    /// **One gap, named.** A `safe_z` change makes the stored RESULTS
    /// stale, and the instrument for that is `drop_all_results`. This
    /// door does not call it. The GUI post panel writes this block on
    /// every frame its widget differs, with no draft-commit step, so a
    /// results drop here destroys generated work while the operator
    /// drags the spinner. The panel needs the draft-commit the stock
    /// panel has before that drop is safe.
    #[instrument(skip(self, post))]
    pub(crate) fn set_post_config(&mut self, post: crate::gcode::PostConfig) -> Effects {
        let reaches_motion = post_change_reaches_motion(&self.post, &post);
        self.with_effects(None, move |session| {
            session.post = post;
            if reaches_motion {
                session.drop_simulation(SimulationDropCause::PostSettings);
            }
        })
    }

    /// Replace the whole machine profile.
    ///
    /// The door of the `SetMachine` command row (§19 ruling 2), which
    /// serves the MCP library snapshot and the GUI machine panel alike.
    ///
    /// It clears the simulation, as [`Self::invalidate_machine`] does.
    /// **This is a behaviour change.**
    /// Before WP4 the method wrote the field and invalidated nothing, so
    /// a stale simulation survived a profile swap; every caller then had
    /// to remember a separate invalidation call.
    ///
    /// B7: a profile that moves the link envelope also drops the results
    /// whose generation read it ([`Self::drop_link_readers_if_moved`]).
    #[instrument(skip(self, machine))]
    pub(crate) fn set_machine(&mut self, machine: crate::machine::MachineProfile) -> Effects {
        self.with_effects(None, move |session| {
            let before = session.machine.link_kinematics();
            let head_before = session.machine.head.fixed_digest();
            session.machine = machine;
            session.drop_simulation(SimulationDropCause::Machine);
            session.drop_link_readers_if_moved(&before);
            session.drop_head_readers_if_moved(head_before);
        })
    }

    /// Write the machine head (H1).
    ///
    /// The door of the `SetMachineHead` command row. It refuses a head that
    /// breaks a rule of [`crate::machine::head::MachineHead::validate`]. It
    /// clears the simulation: the check reads the head. A change of the
    /// FIXED parts also drops every result, because the obstacle step of
    /// every operation reads them ([`Self::drop_head_readers_if_moved`]).
    /// A shoe-only change moves no toolpath.
    ///
    /// # Errors
    /// [`SessionError::InvalidParam`] with the refusal text.
    #[instrument(skip(self, head))]
    pub(crate) fn set_machine_head(
        &mut self,
        head: crate::machine::head::MachineHead,
    ) -> Result<Effects, SessionError> {
        head.validate()
            .map_err(|e| SessionError::InvalidParam(e.to_string()))?;
        Ok(self.with_effects(None, move |session| {
            let head_before = session.machine.head.fixed_digest();
            session.machine.head = head;
            session.drop_simulation(SimulationDropCause::Machine);
            session.drop_head_readers_if_moved(head_before);
        }))
    }

    /// H1: drop every result when the fixed head moved. The obstacle step,
    /// the link ceiling and the shelf floor of each operation read the
    /// fixed parts, so any result can move with them.
    fn drop_head_readers_if_moved(&mut self, before: u64) {
        if self.machine.head.fixed_digest() == before {
            return;
        }
        let all: Vec<usize> = self.results.keys().copied().collect();
        if !all.is_empty() {
            self.drop_results_and_their_dependents(&all);
        }
    }

    /// B7: drop the result of every toolpath whose generation read the
    /// link envelope, when the machine edit moved that envelope.
    ///
    /// The link stage costs each link against a retract with
    /// `MachineProfile::link_kinematics`, so its emitted links can move
    /// with the envelope. `Capabilities::reads_link_kinematics` names those
    /// operations. The air-cut filter times each bridge with the same
    /// envelope (operator ruling 2026-10-15), so a result whose generation
    /// read a stock snapshot (`ToolpathStats::stock_snapshot`) reads it
    /// too. Every other toolpath keeps its result. A change that leaves the
    /// envelope equal (the name, the spindle, the power) drops nothing.
    fn drop_link_readers_if_moved(&mut self, before: &crate::machine::kinematics::LinkKinematics) {
        if self.machine.link_kinematics() == *before {
            return;
        }
        let readers: Vec<usize> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(index, tc)| {
                tc.operation.capabilities().reads_link_kinematics()
                    || self
                        .results
                        .get(index)
                        .is_some_and(|r| r.stats.stock_snapshot.is_some())
            })
            .map(|(index, _)| index)
            .collect();
        if !readers.is_empty() {
            self.drop_results_and_their_dependents(&readers);
        }
    }

    /// Write the machine's kinematics block.
    ///
    /// The door of the `SetMachineKinematics` command row. The caller
    /// supplies the finished block: the merge of a partial per-axis
    /// triple onto the machine's current limits, and the refusal of a
    /// non-positive value, belong to the surface that collected the
    /// numbers.
    ///
    #[instrument(skip(self, kinematics))]
    pub(crate) fn set_machine_kinematics(
        &mut self,
        kinematics: crate::machine::kinematics::MachineKinematics,
    ) -> Effects {
        self.with_effects(None, move |session| {
            let before = session.machine.link_kinematics();
            session.machine.kinematics = Some(kinematics);
            session.drop_simulation(SimulationDropCause::Machine);
            session.drop_link_readers_if_moved(&before);
        })
    }

    /// Write the machine fields a GRBL `$$` dump states.
    ///
    /// The door of the `ImportMachineSettings` command row. A dump
    /// carries more than [`Self::set_machine_kinematics`] — the travel
    /// rate and the controller facts — so the two rows carry two payloads
    /// (§15 ruling 6).
    ///
    /// B6: the import writes only the fields the dump states
    /// ([`GrblImport::apply_to`](crate::machine::kinematics::GrblImport::apply_to)).
    /// Every other field keeps its value: a bare `$$` dump keeps the
    /// firmware, the spindle plugin and the probe distance an earlier
    /// `$I` import gave.
    ///
    /// The parse belongs to the caller
    /// (`MachineKinematics::from_grbl_settings`), which also decides
    /// whether the dump was recognised at all.
    ///
    /// G10: a dump that names its firmware (grblHAL) also sets the post
    /// dialect, so the machine profile, not a separate project choice,
    /// selects the dialect. The dialect is read at emit time and drops
    /// nothing more than the machine change already does.
    #[instrument(skip(self, import))]
    pub(crate) fn import_machine_settings(
        &mut self,
        import: &crate::machine::kinematics::GrblImport,
    ) -> Effects {
        self.with_effects(None, move |session| {
            let before = session.machine.link_kinematics();
            import.apply_to(&mut session.machine);
            if let Some(format) = import.controller.firmware.post_format() {
                session.post.format = format;
            }
            session.drop_simulation(SimulationDropCause::Machine);
            session.drop_link_readers_if_moved(&before);
        })
    }

    /// Replace one tool, addressed by its id.
    ///
    /// The door of the `ReplaceTool` command row. The GUI tool panel
    /// edits a DRAFT clone and commits the whole draft on Apply, so the
    /// payload is a whole [`ToolConfig`] and not one named parameter.
    ///
    /// It drops the result of every toolpath the tool machines, through
    /// [`Self::drop_tool_results`] — the same rule
    /// [`Self::set_tool_param`] and [`Self::invalidate_tool`] apply, so
    /// the three routes cannot drop different sets. The id is the
    /// project-assigned one, NOT a position in the tools list.
    ///
    /// F029: refuses a tool outside its domain and writes nothing.
    #[instrument(skip(self, tool))]
    pub(crate) fn replace_tool(
        &mut self,
        tool_id: usize,
        tool: ToolConfig,
    ) -> Result<Effects, SessionError> {
        crate::session::validate_tool(&tool)?;
        self.try_with_effects(None, move |session| {
            let Some(slot) = session.tools.iter_mut().find(|t| t.id == ToolId(tool_id)) else {
                return Err(SessionError::ToolNotFound(ToolId(tool_id)));
            };
            // A change of `size_units` alone is display only: it moves how
            // the size is SHOWN, no stored length, so no result is dropped.
            let display_only = slot.size_units != tool.size_units && {
                let mut same_units = slot.clone();
                same_units.size_units = tool.size_units;
                same_units == tool
            };
            *slot = tool;
            if !display_only {
                session.drop_tool_results(tool_id);
            }
            Ok(())
        })
    }

    /// Replace the full tools list.
    ///
    /// Invalidates simulation (tool geometry changes affect material removal).
    ///
    /// F029: refuses the whole list when one tool is outside its domain,
    /// and writes nothing.
    #[instrument(skip(self, tools))]
    pub(crate) fn replace_tools(
        &mut self,
        tools: Vec<ToolConfig>,
    ) -> Result<Effects, SessionError> {
        for tool in &tools {
            crate::session::validate_tool(tool)?;
        }
        Ok(self.with_effects(None, move |session| {
            session.tools = tools;
            // Update the next-ID counter so newly added tools don't
            // collide.
            session.next_tool_id = session.tools.iter().map(|t| t.id.0 + 1).max().unwrap_or(0);
            session.drop_simulation(SimulationDropCause::Operations);
        }))
    }
}
