//! CRUD mutation methods on [`ProjectSession`].
//!
//! Every mutation here reports [`Effects`] — the toolpath indices whose
//! generation inputs moved, whether it cleared the simulation, and the
//! revision of the one toolpath it names. It builds none of that itself:
//! it runs its body inside
//! [`ProjectSession::try_with_effects`](ProjectSession::try_with_effects),
//! the one construction site (`session/command.rs`).

use std::collections::BTreeSet;

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, BoundarySource, DressupConfig, HeightsConfig};
use crate::compute::stock_config::{FixtureId, KeepOutId, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::compute::transform::FaceUp;
use crate::enriched_mesh::FaceGroupId;

use super::{
    Effects, Fixture, KeepOutZone, ProjectPostConfig, ProjectSession, SessionError, SetupData,
    ToolpathConfig,
};
use crate::compute::transform::ZRotation;
use crate::geo::{BoundingBox3, P3};
use crate::polygon::Polygon2;

/// Whether a fixture edit moved something a collision check reads.
///
/// Every field except the NAME is an input of the holder-clearance and
/// fixture-collision checks. The name reaches the setup sheet alone, so
/// a rename must not drop a result (WP6).
///
/// It compares the whole record with the name equalised, so a field
/// ADDED to `Fixture` later counts as a collision input until someone
/// states otherwise. That is the safe default.
fn fixture_collision_inputs_moved(before: &Fixture, after: &Fixture) -> bool {
    let mut named_before = before.clone();
    named_before.name.clone_from(&after.name);
    named_before != *after
}

/// Whether a keep-out edit moved something a collision check reads.
///
/// The twin of [`fixture_collision_inputs_moved`], and it exempts the
/// same one field.
fn keep_out_collision_inputs_moved(before: &KeepOutZone, after: &KeepOutZone) -> bool {
    let mut named_before = before.clone();
    named_before.name.clone_from(&after.name);
    named_before != *after
}

/// Compute a 3D bounding box from a slice of 2D polygons (SVG/DXF models).
/// Z extent is zero; `update_from_bbox` preserves stock Z for 2D models.
/// Returns `None` if all polygons are empty.
pub(crate) fn polygons_bbox(polygons: &[Polygon2]) -> Option<BoundingBox3> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for poly in polygons {
        for pt in poly
            .exterior
            .iter()
            .chain(poly.holes.iter().flat_map(|h| h.iter()))
        {
            if pt.x < min_x {
                min_x = pt.x;
            }
            if pt.y < min_y {
                min_y = pt.y;
            }
            if pt.x > max_x {
                max_x = pt.x;
            }
            if pt.y > max_y {
                max_y = pt.y;
            }
        }
    }
    if !min_x.is_finite() {
        return None;
    }
    Some(BoundingBox3 {
        min: P3::new(min_x, min_y, 0.0),
        max: P3::new(max_x, max_y, 0.0),
    })
}

impl ProjectSession {
    // ── Toolpath CRUD ─────────────────────────────────────────────

    /// Add a toolpath to the specified setup.
    ///
    /// [`Effects::created`] carries the new toolpath's index in
    /// `toolpath_configs`, the value this method returned before WP4.
    /// [`Effects::revision`] is `None`: the caller names no existing
    /// index, so no revision read answers about it.
    #[instrument(skip(self, config))]
    pub fn add_toolpath(
        &mut self,
        setup_index: usize,
        config: ToolpathConfig,
    ) -> Result<Effects, SessionError> {
        let mut created = None;
        // The error type is named because this call is not in tail
        // position: the `?` below leaves `E` constrained by two `From`
        // bounds alone, which the compiler cannot solve. `with_effects`
        // names `Infallible` for the same reason.
        let mut effects = self.try_with_effects(None, |session| {
            created = Some(session.add_toolpath_impl(setup_index, config)?);
            Ok::<(), SessionError>(())
        })?;
        effects.created = created;
        Ok(effects)
    }

    /// Append the toolpath and report its index. The raw half of
    /// [`Self::add_toolpath`], which wraps it in the one [`Effects`]
    /// construction site.
    pub(crate) fn add_toolpath_impl(
        &mut self,
        setup_index: usize,
        mut config: ToolpathConfig,
    ) -> Result<usize, SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;

        // Assign a fresh ID
        config.id = crate::ids::ToolpathId(self.next_toolpath_id);
        self.next_toolpath_id += 1;

        let tp_index = self.toolpath_configs.len();
        self.toolpath_configs.push(config);
        setup.toolpath_indices.push(tp_index);

        // Adding a toolpath invalidates simulation
        self.simulation = None;

        Ok(tp_index)
    }

    /// Remove a toolpath by its index in `toolpath_configs`.
    ///
    /// Updates all setup `toolpath_indices` so that indices above the
    /// removed one are shifted down by one.
    ///
    /// [`Effects::revision`] is `None`: the index names a different
    /// toolpath after the removal, so no revision read at it answers
    /// about the toolpath the caller passed.
    #[instrument(skip(self))]
    pub fn remove_toolpath(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, |session| {
            if index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(index));
            }

            // Propagate BEFORE the removal reshuffles indices: downstream
            // rest-machining results were generated against the stock this
            // toolpath left (see `invalidate_result_chain`).
            session.invalidate_output_dependents(index, true);

            session.toolpath_configs.remove(index);
            session.drop_result(index);

            // Rebuild every setup's toolpath_indices: remove the index,
            // then decrement any index above it.
            for setup in &mut session.setups {
                setup.toolpath_indices.retain(|&i| i != index);
                for idx in &mut setup.toolpath_indices {
                    if *idx > index {
                        *idx -= 1;
                    }
                }
            }

            // Re-key cached results whose index shifted
            let mut new_results = std::collections::HashMap::new();
            for (k, v) in session.results.drain() {
                if k > index {
                    new_results.insert(k - 1, v);
                } else {
                    new_results.insert(k, v);
                }
            }
            session.results = new_results;

            // Every index above the removed one now names a different
            // toolpath, so no revision a reader recorded still applies.
            session.bump_all_revisions();

            session.simulation = None;
            Ok(())
        })
    }

    /// Move a toolpath from `from_index` to `to_index` within the same setup.
    ///
    /// Both indices refer to positions in the session-level `toolpath_configs`
    /// vec. The toolpath must belong to the same setup as determined by the
    /// current `toolpath_indices`.
    ///
    /// This is an INSERT, not a swap (G-DROPINDEX, 2026-09-10): the moved op
    /// takes the target op's place in plan order and everything between the
    /// two shifts by one. It used to swap the two positions, which the
    /// operator sees only when the two are not adjacent — dragging the last
    /// op to the top also sent the top op to the bottom.
    ///
    /// The two affordances that reach this method stay consistent under the
    /// change. Move Up / Move Down pass the ADJACENT op's index, and for
    /// adjacent positions an insert and a swap are the same permutation, so
    /// those two menu items are byte-identical before and after. Drag-and-drop
    /// carries the position the operator pointed at, which is an insertion
    /// gap, and only an insert can honour it. They are two gestures with one
    /// meaning — "put this op here" — not two policies that had to be
    /// reconciled.
    ///
    /// [`Effects::revision`] is `None`: the call names two toolpaths, so
    /// there is no one index to report a revision for.
    #[instrument(skip(self))]
    pub fn reorder_toolpath(
        &mut self,
        from_index: usize,
        to_index: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, |session| {
            if from_index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(from_index));
            }
            if to_index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(to_index));
            }
            if from_index == to_index {
                return Ok(());
            }

            // Re-order only the display order within the setup that owns
            // both toolpaths. `toolpath_configs` and the results cache are
            // keyed by stable global indices and must not move.
            //
            // Remove at `pf`, insert at `pt`, both read off the ORIGINAL
            // list. Downward (pf < pt) that lands the op just after the
            // target, upward (pf > pt) just before it; either way the
            // target keeps its neighbours and only the moved op changes
            // rank.
            for setup in &mut session.setups {
                let pos_from = setup.toolpath_indices.iter().position(|&i| i == from_index);
                let pos_to = setup.toolpath_indices.iter().position(|&i| i == to_index);
                if let (Some(pf), Some(pt)) = (pos_from, pos_to) {
                    let moved = setup.toolpath_indices.remove(pf);
                    setup.toolpath_indices.insert(pt, moved);
                    break;
                }
            }

            // Plan order changed for both toolpaths: each moved op's own
            // FromRemainingStock result (if any) and everything downstream
            // of either position may have been generated against a stock
            // sequence that no longer exists (see
            // `invalidate_result_chain`). Each moved op is covered as
            // "downstream" of the other's seed.
            session.invalidate_output_dependents(from_index, true);
            session.invalidate_output_dependents(to_index, true);

            session.simulation = None;
            Ok(())
        })
    }

    /// Invalidate a toolpath's cached result plus every cached result that
    /// consumed its outputs, then drop the simulation.
    ///
    /// Dependency closure (2026-07-09, the live-v2 staleness collision
    /// class): a `FromRemainingStock` toolpath's geometry — including the
    /// `optimize_entry_descents` rapids lowered to `prior stock clearance
    /// ceiling + 2 mm` (that ceiling was the flat-disc material top until
    /// B2 made it profile-aware, so a descent may now sit legitimately
    /// BELOW the prior stock top; the invalidation below keys on the stock
    /// CHAIN, never on a height, and is unaffected)
    /// — is generated against the stock its UPSTREAM ops leave. When an
    /// upstream op's material-removal contribution changes (param edit,
    /// enable/disable, reorder, removal) and the downstream result is kept,
    /// the next simulation runs those baked descents against a stock that
    /// no longer matches: the GUI incident showed 20 rapid collisions at
    /// z = 19.996 (old finished ceiling 17.996 + 2 mm clearance) grazing
    /// the raw stock top at 20 — a real gouge hazard at rapid feed, not an
    /// instrument artifact. Cached results that can no longer be trusted
    /// must die with the edit:
    ///
    /// - every LATER toolpath in the same setup (plan order) with
    ///   `StockSource::FromRemainingStock`, downstream of any op whose
    ///   stock contribution changed;
    /// - every toolpath whose enabled `DerivedRestRegions` boundary source
    ///   is an invalidated toolpath (its clip regions came from that cached
    ///   result);
    /// - transitively, to fixpoint (an invalidated rest op's own output
    ///   feeds further rest ops).
    ///
    /// `stock_chain_changed`: whether THIS toolpath's contribution to the
    /// setup's material-removal sequence changed. Pass `true` from content
    /// edits on an enabled toolpath and from enable/disable flips (both
    /// directions change the chain); pass `false` for edits to a toolpath
    /// that is and stays disabled (only its `DerivedRestRegions` consumers
    /// are invalidated).
    ///
    /// Returns every index whose cached result this call dropped. The set
    /// always holds `index` itself, because `drop_result` runs on it
    /// whether or not it held a cached result.
    pub(crate) fn invalidate_result_chain(
        &mut self,
        index: usize,
        stock_chain_changed: bool,
    ) -> BTreeSet<usize> {
        self.drop_result(index);
        let (_dirty, mut dropped) = self.invalidate_output_dependents(index, stock_chain_changed);
        dropped.insert(index);
        dropped
    }

    /// Propagation half of [`Self::invalidate_result_chain`]: invalidate
    /// consumers of `index`'s outputs WITHOUT touching `index`'s own cached
    /// result. Used by the enable toggle — the toggled op's own result stays
    /// valid for a future re-enable; only what was built on top of its
    /// stock/regions is stale.
    ///
    /// Returns `(dirty, dropped)`. `dirty` is every index whose cached
    /// result is now invalid, INCLUDING `index` itself. `dropped` is the
    /// subset this call actually called `drop_result` on, which EXCLUDES
    /// `index`: the enable toggle depends on exactly that asymmetry, so
    /// the two sets are reported apart rather than conflated.
    pub(crate) fn invalidate_output_dependents(
        &mut self,
        index: usize,
        stock_chain_changed: bool,
    ) -> (BTreeSet<usize>, BTreeSet<usize>) {
        self.simulation = None;

        // Indices whose cached result is now invalid.
        let mut dirty: BTreeSet<usize> = BTreeSet::new();
        dirty.insert(index);
        // The subset `drop_result` ran on. `index` never joins it here.
        let mut dropped: BTreeSet<usize> = BTreeSet::new();
        // Subset of `dirty` whose stock contribution changed (drives the
        // same-setup downstream rule). A dirtied enabled op joins this set:
        // its regenerated output may cut differently.
        let mut chain_dirty: BTreeSet<usize> = BTreeSet::new();
        if stock_chain_changed {
            chain_dirty.insert(index);
        }

        loop {
            let mut newly: Vec<usize> = Vec::new();

            // (a) same-setup plan-order downstream FromRemainingStock ops.
            for setup in &self.setups {
                let mut upstream_changed = false;
                for &tp_idx in &setup.toolpath_indices {
                    if chain_dirty.contains(&tp_idx) {
                        upstream_changed = true;
                        continue;
                    }
                    if upstream_changed
                        && !dirty.contains(&tp_idx)
                        && self.toolpath_configs.get(tp_idx).is_some_and(|tc| {
                            tc.enabled
                                && matches!(
                                    tc.stock_source,
                                    crate::session::StockSource::FromRemainingStock
                                )
                        })
                    {
                        newly.push(tp_idx);
                    }
                }
            }

            // (b) DerivedRestRegions consumers of any dirty toolpath.
            let dirty_ids: BTreeSet<crate::ids::ToolpathId> = dirty
                .iter()
                .filter_map(|&i| self.toolpath_configs.get(i).map(|tc| tc.id))
                .collect();
            for (tp_idx, tc) in self.toolpath_configs.iter().enumerate() {
                if dirty.contains(&tp_idx) || !tc.boundary.enabled {
                    continue;
                }
                if let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                    &tc.boundary.source
                    && dirty_ids.contains(source_toolpath_id)
                {
                    newly.push(tp_idx);
                }
            }

            if newly.is_empty() {
                break;
            }
            for tp_idx in newly {
                self.drop_result(tp_idx);
                dirty.insert(tp_idx);
                dropped.insert(tp_idx);
                if self
                    .toolpath_configs
                    .get(tp_idx)
                    .is_some_and(|tc| tc.enabled)
                {
                    chain_dirty.insert(tp_idx);
                }
            }
        }

        (dirty, dropped)
    }

    /// Enable or disable a toolpath. Flipping the flag in EITHER direction
    /// changes the setup's material-removal chain, so downstream
    /// rest-machining results are invalidated (see
    /// [`Self::invalidate_result_chain`]).
    ///
    /// The toggled op KEEPS its own cached result (the N1 design): it
    /// stays valid for a re-enable. Its revision therefore does not move,
    /// and [`Effects::stale`] — the revision-moved set — excludes
    /// `index`. It reports the downstream set only.
    #[instrument(skip(self))]
    pub fn set_toolpath_enabled(
        &mut self,
        index: usize,
        enabled: bool,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            let changed = tc.enabled != enabled;
            tc.enabled = enabled;
            if changed {
                session.invalidate_output_dependents(index, true);
            }
            session.simulation = None;
            Ok(())
        })
    }

    /// Replace the dressup config for a toolpath, invalidating its cached result.
    #[instrument(skip(self, dressups))]
    pub fn set_dressup_config(
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

    /// Replace a toolpath's operation config wholesale — including switching
    /// the operation KIND — while keeping its tool, heights, boundary,
    /// dressups, and position in the machining order. This is the supported
    /// way to A/B one operation against another in an existing chain
    /// (chain order matters: rest-referencing ops downstream see the stock
    /// this toolpath leaves). Re-normalizes the dressups for the new op kind
    /// and drops the cached result + simulation.
    #[instrument(skip(self, operation))]
    pub fn set_toolpath_operation(
        &mut self,
        index: usize,
        operation: OperationConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.operation = operation;
            // Same invariant set_dressup_config enforces: the surviving
            // dressups must be legal for the NEW operation kind.
            let mut dressups = tc.dressups.clone();
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
    pub fn set_dressup_field(
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
                if !obj.contains_key(key) {
                    return Err(SessionError::InvalidParam(format!(
                        "unknown dressup field '{key}'"
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
    pub fn set_stock_source(
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
    pub fn set_heights_config(
        &mut self,
        index: usize,
        heights: HeightsConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.heights = heights;
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
    pub fn set_boundary_config(
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
    pub fn auto_enable_rest_analysis_for_source(
        &mut self,
        source_id: crate::ids::ToolpathId,
    ) -> Option<Effects> {
        let (idx, tc) = self.find_toolpath_config_by_id(source_id)?;
        if tc.rest_analysis.enabled {
            return None;
        }
        if let OperationConfig::Pencil(cfg) = &tc.operation
            && crate::pencil::PencilDetector::parse(&cfg.detector)
                == crate::pencil::PencilDetector::RestDepth
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
    pub fn set_rest_analysis_config(
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

    // ── Model CRUD ────────────────────────────────────────────────

    /// Add a model and return its ID.
    ///
    /// When `stock.auto_from_model` is enabled, the stock dimensions are
    /// auto-updated from the new model's bounding box — mesh bbox for
    /// STL/STEP models, polygon bbox for SVG/DXF. Without this, MCP
    /// users who called `import_model` after loading a project saw the
    /// stock stay at its pre-import size (see F-13 in the April review).
    ///
    /// For 2D polygon models (zero Z extent), `update_from_bbox`
    /// preserves the existing stock Z dimension rather than collapsing
    /// it to 0.
    ///
    /// [`Effects::created`] carries the new model's ID — not its index.
    /// A model is named by id on every other surface, and this method
    /// answered with the id before WP4.
    #[instrument(skip(self, model))]
    pub fn add_model(&mut self, model: super::LoadedModel) -> Effects {
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_model_impl(model));
        });
        effects.created = created;
        effects
    }

    /// Append the model and report its ID. The raw half of
    /// [`Self::add_model`].
    pub(crate) fn add_model_impl(&mut self, mut model: super::LoadedModel) -> usize {
        model.id = self.next_model_id;
        self.next_model_id += 1;
        let id = model.id;

        // Compute bbox before moving the model into self.models.
        let auto_from_model = self.stock.auto_from_model;
        let model_bbox = if auto_from_model {
            model.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                model
                    .polygons
                    .as_ref()
                    .and_then(|polys| polygons_bbox(polys))
            })
        } else {
            None
        };

        self.models.push(model);

        if let Some(bbox) = model_bbox {
            self.stock.update_from_bbox(&bbox);
        }

        id
    }

    /// Remove a model by index.
    ///
    /// The removal drops no toolpath result. A toolpath binds a model by
    /// id, so a bound toolpath refuses to generate rather than reading
    /// the wrong geometry.
    #[instrument(skip(self))]
    pub fn remove_model(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if index >= session.models.len() {
                return Err(SessionError::MissingGeometry(format!(
                    "Model index {index} not found"
                )));
            }
            session.models.remove(index);
            Ok(())
        })
    }

    // ── Tool CRUD ─────────────────────────────────────────────────

    /// Add a tool.
    ///
    /// [`Effects::created`] carries the new tool's index in the tools
    /// list, the value this method returned before WP4. That index is
    /// NOT the tool's id: a project that has removed a tool separates
    /// the two.
    #[instrument(skip(self, config))]
    pub fn add_tool(&mut self, config: ToolConfig) -> Effects {
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_tool_impl(config));
        });
        effects.created = created;
        effects
    }

    /// Append the tool and report its index. The raw half of
    /// [`Self::add_tool`].
    pub(crate) fn add_tool_impl(&mut self, mut config: ToolConfig) -> usize {
        config.id = ToolId(self.next_tool_id);
        self.next_tool_id += 1;
        let idx = self.tools.len();
        self.tools.push(config);
        idx
    }

    /// Remove a tool by index. Errors if any toolpath still references it.
    #[instrument(skip(self))]
    pub fn remove_tool(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let tool = session
                .tools
                .get(index)
                .ok_or(SessionError::ToolNotFound(ToolId(index)))?;
            let tool_raw_id = tool.id.0;

            // Check no toolpaths reference this tool
            let in_use = session
                .toolpath_configs
                .iter()
                .any(|tc| tc.tool_id == tool_raw_id);
            if in_use {
                return Err(SessionError::ToolInUse(ToolId(tool_raw_id)));
            }

            session.tools.remove(index);
            Ok(())
        })
    }

    // ── Setup CRUD ────────────────────────────────────────────────

    /// Add a new setup.
    ///
    /// [`Effects::created`] carries the new setup's index in the setups
    /// list, the value this method returned before WP4.
    #[instrument(skip(self))]
    pub fn add_setup(&mut self, name: String, face_up: FaceUp) -> Effects {
        let mut created = None;
        let mut effects = self.with_effects(None, |session| {
            created = Some(session.add_setup_impl(name, face_up));
        });
        effects.created = created;
        effects
    }

    /// Append the setup and report its index. The raw half of
    /// [`Self::add_setup`].
    pub(crate) fn add_setup_impl(&mut self, name: String, face_up: FaceUp) -> usize {
        let id = self.next_setup_id;
        self.next_setup_id += 1;
        let idx = self.setups.len();
        self.setups.push(SetupData {
            id,
            name,
            face_up,
            z_rotation: ZRotation::default(),
            datum: crate::session::DatumConfig::default(),
            model_ids: Vec::new(),
            fixtures: Vec::new(),
            keep_out_zones: Vec::new(),
            toolpath_indices: Vec::new(),
            pause_message: None,
        });
        idx
    }

    /// Remove a setup by index. Errors if the setup still has toolpaths.
    ///
    /// An empty setup owns no result, so the removal drops none.
    #[instrument(skip(self))]
    pub fn remove_setup(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get(index)
                .ok_or(SessionError::SetupNotFound(index))?;
            if !setup.toolpath_indices.is_empty() {
                return Err(SessionError::SetupHasToolpaths(index));
            }
            session.setups.remove(index);
            Ok(())
        })
    }

    // ── Cross-setup moves ─────────────────────────────────────────

    /// Move a toolpath from its current setup to a different setup.
    ///
    /// `target_position` is the gap the toolpath lands in, counted in the
    /// TARGET setup's own plan order: `Some(0)` puts it first, `Some(len)`
    /// or anything larger puts it last, and `None` appends. It is an
    /// `Option` rather than a plain index because the two callers ask
    /// different questions — a drag carries the drop position the operator
    /// pointed at, while MCP `move_toolpath_to_setup` has no position
    /// argument at all and must keep appending (G-DROPINDEX, 2026-09-10:
    /// the drop index used to be computed by the panel, passed through the
    /// event and then dropped on the floor here, so every cross-setup drag
    /// landed at the bottom of the target setup whatever the operator did).
    ///
    /// The cached toolpath result is invalidated because the setup transform
    /// may have changed (e.g. top → bottom orientation).
    ///
    /// [`Effects::revision`] is `None`: the toolpath lands in another
    /// setup's plan order, so the index the caller passed no longer
    /// describes where the toolpath sits.
    #[instrument(skip(self))]
    pub fn move_toolpath_to_setup(
        &mut self,
        tp_index: usize,
        target_setup_index: usize,
        target_position: Option<usize>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if tp_index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(tp_index));
            }
            if target_setup_index >= session.setups.len() {
                return Err(SessionError::SetupNotFound(target_setup_index));
            }

            // Propagate while the toolpath is still in its ORIGINAL setup
            // so that setup's downstream rest ops are invalidated (see
            // `invalidate_result_chain`); its own result dies below.
            session.invalidate_output_dependents(tp_index, true);

            // Remove from whichever setup currently owns this toolpath
            for setup in &mut session.setups {
                setup.toolpath_indices.retain(|&i| i != tp_index);
            }

            // SAFETY: target_setup_index bounds-checked above
            #[allow(clippy::indexing_slicing)]
            let target = &mut session.setups[target_setup_index].toolpath_indices;
            let at = target_position.unwrap_or(target.len()).min(target.len());
            target.insert(at, tp_index);

            session.drop_result(tp_index);
            session.simulation = None;
            Ok(())
        })
    }

    // ── Toolpath config updates ──────────────────────────────────

    /// Rebind a toolpath to a different **tool**, invalidating its cached
    /// result and everything downstream of the stock it leaves.
    ///
    /// `tool_id` is the project-assigned [`ToolConfig::id`] (the `id` field
    /// of a `list_tools` row), NOT a positional index — it is the same
    /// number [`crate::session::ToolpathConfig::tool_id`] stores.
    ///
    /// This is a BINDING change, not an operation parameter: no operation
    /// config carries a `tool_id` field, so
    /// [`ProjectSession::set_toolpath_param`](Self::set_toolpath_param)
    /// cannot reach it and refuses the key. Keep the two routes disjoint —
    /// `set_toolpath_param` writes the operation's own params through a
    /// serde round-trip, and Rest's `prev_tool_id` IS one of those params
    /// (the rest-analysis reference tool), which is a different thing from
    /// the cutter this toolpath runs.
    ///
    /// The tool must exist. Its SHAPE is deliberately not checked here: an
    /// operation with a tool its registry entry rejects is a *blocked*
    /// operation the operator can fix by rebinding, and refusing the
    /// rebind would remove the only fix. The shape refusal stays where it
    /// is — `ToolConstraintsDef::allows`, read by the generators.
    ///
    /// A rebind to the tool already bound is a no-op and invalidates
    /// nothing.
    #[instrument(skip(self))]
    pub fn set_toolpath_tool(
        &mut self,
        index: usize,
        tool_id: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            if !session.tools.iter().any(|t| t.id.0 == tool_id) {
                return Err(SessionError::ToolNotFound(ToolId(tool_id)));
            }
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.tool_id == tool_id {
                return Ok(());
            }
            tc.tool_id = tool_id;
            let enabled = tc.enabled;
            // A different cutter removes different material, so this is a
            // stock-chain change: same invalidation as
            // `set_face_selection` and `set_heights_config`, not the
            // narrower `remove_result`.
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Rebind a toolpath to a different **input model**, invalidating its
    /// cached result and everything downstream of the stock it leaves.
    ///
    /// `model_id` is the project-assigned [`super::LoadedModel::id`] — the
    /// `id` field of an `inspect_model` row, NOT a positional index. It is
    /// the same number [`crate::session::ToolpathConfig::model_id`] stores.
    ///
    /// The model must exist. Unlike `add_toolpath`, which accepts an
    /// unresolvable `model_id` and leaves the operation carrying a dangling
    /// reference, this setter refuses one: the whole point of the route is
    /// to REPAIR a binding.
    ///
    /// The geometry KIND is deliberately not checked (a 2D-only operation
    /// may be pointed at a mesh). That precondition belongs to the refusal
    /// contract, not to this setter.
    ///
    /// **Caveat this does not handle:** `face_selection` holds
    /// [`FaceGroupId`]s of the model that was bound when the faces were
    /// picked. A rebind leaves them in place, pointing into a different
    /// mesh. Clearing them is a separate decision; the GUI's Input combo
    /// does not clear them either.
    ///
    /// A rebind to the model already bound is a no-op and invalidates
    /// nothing.
    #[instrument(skip(self))]
    pub fn set_toolpath_model(
        &mut self,
        index: usize,
        model_id: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            if !session.models.iter().any(|m| m.id == model_id) {
                let known: Vec<String> = session.models.iter().map(|m| m.id.to_string()).collect();
                return Err(SessionError::MissingGeometry(format!(
                    "Model id {model_id} not found; project model ids: [{}]",
                    known.join(", ")
                )));
            }
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.model_id == model_id {
                return Ok(());
            }
            tc.model_id = model_id;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Set the BREP face selection for a toolpath, invalidating its cached result.
    #[instrument(skip(self, face_ids))]
    pub fn set_face_selection(
        &mut self,
        index: usize,
        face_ids: Option<Vec<FaceGroupId>>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.face_selection = face_ids;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Update the alignment pin drill holes for a toolpath.
    ///
    /// Errors if the toolpath's operation is not `AlignmentPinDrill`.
    #[instrument(skip(self, holes))]
    pub fn set_alignment_pin_drill_holes(
        &mut self,
        index: usize,
        holes: Vec<[f64; 2]>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            match tc.operation {
                OperationConfig::AlignmentPinDrill(ref mut cfg) => {
                    cfg.holes = holes;
                }
                _ => {
                    return Err(SessionError::InvalidParam(
                        "Toolpath is not an AlignmentPinDrill operation".to_owned(),
                    ));
                }
            }
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Set the explicitly-selected drill holes (DXF point / circle-centre
    /// picks) for a `Drill` or `AlignmentPinDrill` toolpath, invalidating its
    /// cached result and the results of every operation downstream of it.
    /// `None` reverts a `Drill` op to its legacy all-polygon-centroids
    /// behaviour.
    ///
    /// A pick changes which holes the operation cuts, so it changes the
    /// stock a downstream `StockSource::FromRemainingStock` operation
    /// reads. This called `drop_result` alone until WP8 (N6), while its
    /// sibling [`Self::set_alignment_pin_drill_holes`] 30 lines above
    /// walked the chain. The two now answer alike.
    ///
    /// Errors if the toolpath's operation is not a drilling op.
    #[instrument(skip(self, selected_holes))]
    pub fn set_drill_selected_holes(
        &mut self,
        index: usize,
        selected_holes: Option<Vec<[f64; 2]>>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            // Read before the match: `tc` is borrowed mutably inside it.
            let enabled = tc.enabled;
            match tc.operation {
                OperationConfig::Drill(ref mut cfg) => {
                    cfg.selected_holes = selected_holes;
                }
                OperationConfig::AlignmentPinDrill(ref mut cfg) => {
                    cfg.selected_holes = selected_holes;
                }
                _ => {
                    return Err(SessionError::InvalidParam(
                        "Toolpath is not a drilling operation".to_owned(),
                    ));
                }
            }
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    // ── Setup mutations ──────────────────────────────────────────

    /// Rename a setup. This is metadata-only and does not affect compute.
    ///
    /// The door of the `SetSetupName` command row. [`Effects::stale`] is
    /// empty by design: the name reaches the M0 pause message and the
    /// setup sheet, never a generation input.
    #[instrument(skip(self))]
    pub fn rename_setup(&mut self, index: usize, name: String) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(index)
                .ok_or(SessionError::SetupNotFound(index))?;
            setup.name = name;
            Ok(())
        })
    }

    /// Replace a setup's datum — how the operator zeroes the machine for
    /// it.
    ///
    /// The door of the `SetSetupDatum` command row. [`Effects::stale`]
    /// is empty by design: every result in a setup is generated in that
    /// setup's own local frame, and the datum reaches the EXPORT alone
    /// (`StockTop` puts Z0 at the stock top of the presented face). A
    /// datum edit therefore moves no geometry a result holds.
    #[instrument(skip(self))]
    pub fn set_setup_datum(
        &mut self,
        setup_index: usize,
        datum: super::DatumConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.datum = datum;
            Ok(())
        })
    }

    /// Replace the message the operator reads at a setup change.
    ///
    /// The door of the `SetSetupPauseMessage` command row.
    /// [`Effects::stale`] is empty by design, on the
    /// [`Self::set_setup_datum`] precedent: the message reaches the
    /// EXPORT alone, beside the `M0` the post emits, so it moves no
    /// geometry a result holds.
    #[instrument(skip(self))]
    pub fn set_setup_pause_message(
        &mut self,
        setup_index: usize,
        message: Option<String>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.pause_message = message;
            Ok(())
        })
    }

    /// Replace the models in scope for a setup.
    ///
    /// The door of the `SetSetupModels` command row. An EMPTY list means
    /// "all models", which is not the same as a list naming every model.
    ///
    /// Drops every result in the setup, like [`Self::set_setup_face`]:
    /// the scope decides which geometry a generation in this setup may
    /// read.
    #[instrument(skip(self))]
    pub fn set_setup_models(
        &mut self,
        setup_index: usize,
        model_ids: Vec<crate::compute::stock_config::ModelId>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.model_ids = model_ids;
            session.drop_setup_results(setup_index);
            Ok(())
        })
    }

    /// Replace one fixture of a setup, keeping its position in the list.
    ///
    /// The door of the `ReplaceFixture` command row. The GUI fixture
    /// panel edits every field of one fixture, so the payload is the
    /// whole record.
    ///
    /// **The write is unconditional; the DROP is gated**, on the
    /// `ReplaceToolpathConfig` precedent. The panel applies one command
    /// per finished edit, and the name is not a collision input — so a
    /// ten-character rename would otherwise drop the setup's results ten
    /// times. Every OTHER field moves the obstacle the holder must
    /// clear, so it drops the setup's results exactly as
    /// [`Self::add_fixture`] and [`Self::remove_fixture`] do.
    ///
    /// **This is a behaviour change.** The panel wrote the fixture in
    /// place and dropped nothing, so a clamp could move under a cached
    /// holder-clearance verdict (G-FRESHSTATE).
    #[instrument(skip(self, fixture))]
    pub fn replace_fixture(
        &mut self,
        setup_index: usize,
        fixture_id: FixtureId,
        fixture: Fixture,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(slot) = setup.fixtures.iter_mut().find(|f| f.id == fixture_id) else {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} carries no fixture with id {}",
                    fixture_id.0
                )));
            };
            let moved = fixture_collision_inputs_moved(slot, &fixture);
            *slot = fixture;
            if moved {
                session.drop_setup_results(setup_index);
            }
            Ok(())
        })
    }

    /// Replace one keep-out zone of a setup, keeping its position.
    ///
    /// The door of the `ReplaceKeepOut` command row, and the twin of
    /// [`Self::replace_fixture`]. It writes unconditionally and drops
    /// the setup's results only when a field other than the name moved,
    /// for the same reason.
    #[instrument(skip(self, zone))]
    pub fn replace_keep_out(
        &mut self,
        setup_index: usize,
        zone_id: KeepOutId,
        zone: KeepOutZone,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            let Some(slot) = setup.keep_out_zones.iter_mut().find(|z| z.id == zone_id) else {
                return Err(SessionError::InvalidParam(format!(
                    "setup {setup_index} carries no keep-out zone with id {}",
                    zone_id.0
                )));
            };
            let moved = keep_out_collision_inputs_moved(slot, &zone);
            *slot = zone;
            if moved {
                session.drop_setup_results(setup_index);
            }
            Ok(())
        })
    }

    /// Add a fixture to a setup, invalidating all toolpath results in that setup.
    #[instrument(skip(self, fixture))]
    pub fn add_fixture(
        &mut self,
        setup_index: usize,
        fixture: Fixture,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.fixtures.push(fixture);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.simulation = None;
            Ok(())
        })
    }

    /// Remove a fixture from a setup by its ID.
    #[instrument(skip(self))]
    pub fn remove_fixture(
        &mut self,
        setup_index: usize,
        fixture_id: FixtureId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.fixtures.retain(|f| f.id != fixture_id);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.simulation = None;
            Ok(())
        })
    }

    /// Add a keep-out zone to a setup, invalidating all toolpath results in that setup.
    #[instrument(skip(self, zone))]
    pub fn add_keep_out(
        &mut self,
        setup_index: usize,
        zone: KeepOutZone,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.keep_out_zones.push(zone);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.simulation = None;
            Ok(())
        })
    }

    /// Remove a keep-out zone from a setup by its ID.
    #[instrument(skip(self))]
    pub fn remove_keep_out(
        &mut self,
        setup_index: usize,
        zone_id: KeepOutId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.keep_out_zones.retain(|z| z.id != zone_id);
            let indices: Vec<usize> = setup.toolpath_indices.clone();
            for &tp_idx in &indices {
                session.drop_result(tp_idx);
            }
            session.simulation = None;
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
    pub fn invalidate_stock(&mut self) -> Effects {
        self.with_effects(None, |session| session.drop_all_results())
    }

    /// Invalidate cached simulation after machine profile was mutated in-place.
    #[instrument(skip(self))]
    pub fn invalidate_machine(&mut self) -> Effects {
        self.with_effects(None, |session| {
            session.simulation = None;
        })
    }

    /// Invalidate cached results for all toolpaths that reference a given
    /// tool. [`Effects::stale`] carries the toolpath indices whose result
    /// was dropped, so a caller can request their regeneration.
    #[instrument(skip(self))]
    pub fn invalidate_tool(&mut self, tool_id: usize) -> Effects {
        self.with_effects(None, move |session| session.drop_tool_results(tool_id))
    }

    /// Drop the cached result of every toolpath that depends on one
    /// tool, plus the simulation.
    ///
    /// The raw half of [`Self::invalidate_tool`].
    /// [`Self::set_tool_param`] calls it directly, so the two routes
    /// cannot drop different sets and neither nests one [`Effects`]
    /// construction inside another.
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
        for &idx in &stale {
            self.drop_result(idx);
        }
        self.simulation = None;
    }

    /// Invalidate cached results for every toolpath that machines a given
    /// model, plus the simulation.
    ///
    /// A model reload or rescale replaces the geometry every one of those
    /// results was generated against, so none of them still describes the
    /// project. [`Effects::stale`] carries the toolpath indices whose
    /// result was dropped, so a caller can request their regeneration.
    #[instrument(skip(self))]
    pub fn invalidate_model(&mut self, model_id: usize) -> Effects {
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
    fn drop_results_for_model(&mut self, model_id: usize) {
        let affected: Vec<usize> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(_, tc)| tc.model_id == model_id)
            .map(|(idx, _)| idx)
            .collect();
        for &idx in &affected {
            self.drop_result(idx);
        }
        self.simulation = None;
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
    pub fn adopt_model_geometry(
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
            model.adopt_geometry(geometry);
            if let Some(units) = units {
                model.units = Some(units);
            }
            // The id did NOT change, so no signature comparison can see
            // this. `generation_inputs_signature` carries `model_id`, and
            // the id is what a refresh keeps. Dropping by id is therefore
            // the only instrument that answers.
            session.drop_results_for_model(model_id);
            Ok(())
        })
    }

    // ── Global config ─────────────────────────────────────────────

    /// Replace the stock configuration.
    ///
    /// Drops every toolpath result — see [`Self::drop_all_results`].
    #[instrument(skip(self, stock))]
    pub fn set_stock_config(&mut self, stock: StockConfig) -> Effects {
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
    pub fn update_stock_from_bbox(&mut self, bbox: &BoundingBox3) -> Effects {
        self.with_effects(None, move |session| {
            session.stock.update_from_bbox(bbox);
            session.drop_all_results();
        })
    }

    /// Add an alignment pin, deduping against existing pins within 0.01mm.
    ///
    /// Reports the effects when the pin was added, and `None` when a
    /// duplicate was skipped. `None` says the call changed nothing.
    #[instrument(skip(self))]
    pub fn add_alignment_pin(&mut self, x: f64, y: f64, diameter: f64) -> Option<Effects> {
        const PIN_DEDUP_EPSILON_MM: f64 = 0.01;
        let exists = self.stock.alignment_pins.iter().any(|p| {
            (p.x - x).abs() < PIN_DEDUP_EPSILON_MM && (p.y - y).abs() < PIN_DEDUP_EPSILON_MM
        });
        if exists {
            return None;
        }
        Some(self.with_effects(None, move |session| {
            session
                .stock
                .alignment_pins
                .push(crate::compute::stock_config::AlignmentPin::new(
                    x, y, diameter,
                ));
            session.drop_all_results();
        }))
    }

    /// Remove an alignment pin by index. Returns `Err` if out of bounds.
    ///
    /// `index` names a PIN, not a toolpath, so [`Effects::revision`] is
    /// `None`.
    #[instrument(skip(self))]
    pub fn remove_alignment_pin(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if index >= session.stock.alignment_pins.len() {
                return Err(SessionError::InvalidParam(format!(
                    "alignment pin index {index} out of range ({} pins)",
                    session.stock.alignment_pins.len()
                )));
            }
            session.stock.alignment_pins.remove(index);
            session.drop_all_results();
            Ok(())
        })
    }

    /// Replace the post-processor configuration, invalidating simulation.
    #[instrument(skip(self, post))]
    pub fn set_post_config(&mut self, post: ProjectPostConfig) -> Effects {
        self.with_effects(None, move |session| {
            session.post = post;
            session.simulation = None;
        })
    }

    /// Replace the whole machine profile.
    ///
    /// The door of the `SetMachine` command row (§19 ruling 2), which
    /// serves the MCP library snapshot and the GUI machine panel alike.
    ///
    /// It invalidates exactly as [`Self::invalidate_machine`] does: it
    /// clears the simulation and drops no toolpath result, because a
    /// machine edit moves no geometry. **This is a behaviour change.**
    /// Before WP4 the method wrote the field and invalidated nothing, so
    /// a stale simulation survived a profile swap; every caller then had
    /// to remember a separate invalidation call.
    ///
    /// The method does NOT clear `machine_ref`. A profile that arrives
    /// from the library is a snapshot of a named machine, and the
    /// caller states whether the link survives. The two GRBL doors,
    /// [`Self::set_machine_kinematics`] and
    /// [`Self::import_machine_settings`], DO clear it: they write inline
    /// numbers that no library entry published.
    #[instrument(skip(self, machine))]
    pub fn set_machine(&mut self, machine: crate::machine::MachineProfile) -> Effects {
        self.with_effects(None, move |session| {
            session.machine = machine;
            session.simulation = None;
        })
    }

    /// Write the machine's kinematics block and drop the library link.
    ///
    /// The door of the `SetMachineKinematics` command row. The caller
    /// supplies the finished block: the merge of a partial per-axis
    /// triple onto the machine's current limits, and the refusal of a
    /// non-positive value, belong to the surface that collected the
    /// numbers.
    ///
    /// The link drops because the values are inline now and no longer
    /// describe the named library machine.
    #[instrument(skip(self, kinematics))]
    pub fn set_machine_kinematics(
        &mut self,
        kinematics: crate::machine_kinematics::MachineKinematics,
    ) -> Effects {
        self.with_effects(None, move |session| {
            session.machine.kinematics = Some(kinematics);
            session.machine_ref = None;
            session.simulation = None;
        })
    }

    /// Write the machine fields a GRBL `$$` dump carries.
    ///
    /// The door of the `ImportMachineSettings` command row. A dump
    /// carries one more field than [`Self::set_machine_kinematics`] —
    /// the travel rate — so the two rows carry two payloads (§15 ruling
    /// 6). `max_feed_mm_min` of `None` means the dump published no
    /// travel rate, and the machine keeps the one it has.
    ///
    /// The parse belongs to the caller
    /// (`MachineKinematics::from_grbl_settings`), which also decides
    /// whether the dump was recognised at all.
    #[instrument(skip(self, kinematics))]
    pub fn import_machine_settings(
        &mut self,
        kinematics: crate::machine_kinematics::MachineKinematics,
        max_feed_mm_min: Option<f64>,
    ) -> Effects {
        self.with_effects(None, move |session| {
            session.machine.kinematics = Some(kinematics);
            if let Some(max_feed) = max_feed_mm_min {
                session.machine.max_feed_mm_min = max_feed;
            }
            session.machine_ref = None;
            session.simulation = None;
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
    #[instrument(skip(self, tool))]
    pub fn replace_tool(
        &mut self,
        tool_id: usize,
        tool: ToolConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let Some(slot) = session.tools.iter_mut().find(|t| t.id == ToolId(tool_id)) else {
                return Err(SessionError::ToolNotFound(ToolId(tool_id)));
            };
            *slot = tool;
            session.drop_tool_results(tool_id);
            Ok(())
        })
    }

    /// Replace the full tools list.
    ///
    /// Invalidates simulation (tool geometry changes affect material removal).
    #[instrument(skip(self, tools))]
    pub fn replace_tools(&mut self, tools: Vec<ToolConfig>) -> Effects {
        self.with_effects(None, move |session| {
            session.tools = tools;
            // Update the next-ID counter so newly added tools don't
            // collide.
            session.next_tool_id = session.tools.iter().map(|t| t.id.0 + 1).max().unwrap_or(0);
            session.simulation = None;
        })
    }

    /// Install an operation, dressup and face-selection triple on the
    /// toolpath at `index`, and drop THAT INDEX ALONE.
    ///
    /// **The narrowness is the point, and no surface may call this.** The
    /// tool-load optimizer's candidate search
    /// (`tool_load/optimize/candidate.rs`) restores a candidate, then
    /// regenerates that one index, then runs a project simulation against
    /// the neighbours' cached results. `candidate.rs` says so at the
    /// simulation call: "Other toolpaths' cached results from baseline
    /// still apply because `generate_toolpath` only touched index
    /// `toolpath_index`." A wide invalidation would drop every downstream
    /// `StockSource::FromRemainingStock` result once PER CANDIDATE, so the
    /// search would regenerate the whole chain at every point of the
    /// search instead of once. `optimize/context.rs`'s
    /// `BaselineRestoreGuard` restores the baseline on drop through the
    /// same path, and a `Drop` cannot report an error, so a wide restore
    /// there would silently leave a dead chain behind every
    /// `optimize_toolpath` call.
    ///
    /// Every OTHER caller takes
    /// [`Command::RestoreToolpathSnapshot`](super::Command), which walks
    /// the chain (WP8, N14). `the_narrow_path_leaves_the_neighbour_result_cached`
    /// in this file's test module pins the difference.
    #[instrument(skip(self, operation, dressups, face_selection))]
    pub(crate) fn apply_toolpath_param_snapshot_narrow(
        &mut self,
        index: usize,
        operation: OperationConfig,
        dressups: DressupConfig,
        face_selection: Option<Vec<FaceGroupId>>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.operation = operation;
            tc.dressups = dressups;
            tc.face_selection = face_selection;
            session.drop_result(index);
            session.simulation = None;
            Ok(())
        })
    }

    /// Overwrite a toolpath's feeds provenance (W2.1), stamping
    /// [`crate::feeds::ProvenanceSource::Optimizer`] on the dimensions a
    /// caller changed. Does not touch the result or simulation caches.
    ///
    /// **This has no production caller since WP8.** The three optimizer
    /// apply paths called it right after the snapshot, which is how an
    /// undo came to restore the earlier values under the later stamp.
    /// They now carry the provenance in
    /// [`RestoreToolpathSnapshotArgs`](super::RestoreToolpathSnapshotArgs),
    /// so one command writes the values and the stamp together.
    pub fn set_feeds_provenance(
        &mut self,
        index: usize,
        feeds_provenance: crate::feeds::FeedsProvenance,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.feeds_provenance = feeds_provenance;
            Ok(())
        })
    }

    /// Replace a toolpath's debug options.
    ///
    /// The door of the `SetToolpathDebugOptions` command row (§16 ruling
    /// 7). The generate door writes this flag immediately before it
    /// runs, so the row moves NO revision and drops no result: a debug
    /// trace is an output of a generation, never an input to one.
    ///
    /// This is not the §9 Q4 divergence. Q4 preserves today's behaviour
    /// for `set_toolpath_param`'s arm alone; a dedicated row for the
    /// generate-time write bumps nothing by design.
    #[instrument(skip(self))]
    pub fn set_toolpath_debug_options(
        &mut self,
        index: usize,
        debug_options: crate::debug_trace::ToolpathDebugOptions,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.debug_options = debug_options;
            Ok(())
        })
    }

    /// Write a freshly-computed `ToolpathComputeResult` into `session.results`.
    ///
    /// Symmetric counterpart to the [`Self::drop_result`] calls
    /// performed by every mutating method on `ProjectSession`. The
    /// invariant is: `session.results[idx]` should always be either
    /// absent (toolpath is stale / never generated) or a fresh result
    /// produced from the current toolpath config. The viz-side threaded
    /// compute backend writes through this method so the core cache
    /// stays in sync with the viz-side `gui.toolpath_rt[id].result`
    /// cache; before this method existed, the two caches diverged after
    /// the undo snapshot path cleared `results[idx]` and the
    /// GUI regen only repopulated `gui.toolpath_rt`. See
    /// `planning/F1_RCA.md` (Roadmap F.1) for the divergence write-up.
    ///
    /// WP3 closed the public door. A completion now arrives as
    /// [`Command::AdoptResult`](crate::session::Command), which carries
    /// the revision the compute lane started from and refuses a
    /// completion that answers a superseded parameter set.
    /// [`ProjectSession::apply`] is the only caller.
    pub(crate) fn insert_result(
        &mut self,
        index: usize,
        result: super::ToolpathComputeResult,
    ) -> Result<(), SessionError> {
        if index >= self.toolpath_configs.len() {
            return Err(SessionError::ToolpathNotFound(index));
        }
        self.results.insert(index, result);
        Ok(())
    }

    /// Drop a toolpath's cached `ToolpathComputeResult`.
    ///
    /// The other half of [`Self::insert_result`]'s contract, for the case
    /// that used to have no half at all: a generation that **failed**. The
    /// viz-side worker path sets `gui.toolpath_rt[id].result = None` on a
    /// compute error, but before this existed nothing cleared the core-side
    /// `session.results[idx]`, so a previous parameter set's toolpath stayed
    /// cached and readable as if it were this configuration's answer —
    /// exported, simulated, and (via
    /// [`crate::compute::simulate::PhantomPriorStockScan`], which asks only
    /// "has this been generated?") counted as generated, which withholds a
    /// pending rest op's phantom prior-stock snapshot.
    ///
    /// Never an error: removing the result of an index that has none is
    /// the no-op the callers want.
    ///
    /// The method reported `true` when a cached result was actually
    /// removed. It reports [`Effects`] instead, and the two are not the
    /// same reading: `drop_result` bumps the revision whether or not a
    /// result was there, so `Effects::stale` always holds `index`. A
    /// caller that needs "was a result present" reads
    /// [`ProjectSession::get_result`] before the call.
    pub fn remove_result(&mut self, index: usize) -> Effects {
        self.with_effects(Some(index), move |session| {
            session.drop_result(index);
        })
    }

    /// Drop a toolpath's cached result and bump its revision.
    ///
    /// **The single site that does either.** Every mutating method on
    /// `ProjectSession` that used to call `self.results.remove(&index)`
    /// calls this instead, so "the cached answer is gone" and "the inputs
    /// moved" are one event with one record. `insert_result` deliberately
    /// does NOT bump: recording an answer is not a change of inputs.
    ///
    /// Returns `true` when a cached result was actually removed. The
    /// revision bumps either way — an edit to a never-generated toolpath
    /// still moves its inputs, and a reader comparing revisions across a
    /// generation must see that.
    pub(crate) fn drop_result(&mut self, index: usize) -> bool {
        self.next_revision += 1;
        self.toolpath_revision.insert(index, self.next_revision);
        self.results.remove(&index).is_some()
    }

    /// Bump every toolpath's revision without touching the result cache.
    ///
    /// Used where toolpath INDICES shift (a removal, a bulk replace): after
    /// such a move an index no longer names the toolpath a reader recorded
    /// a revision for, so every outstanding comparison must fail.
    pub(crate) fn bump_all_revisions(&mut self) {
        for index in 0..self.toolpath_configs.len() {
            self.next_revision += 1;
            self.toolpath_revision.insert(index, self.next_revision);
        }
    }

    /// Drop every toolpath's cached result and the simulation.
    ///
    /// The stock rule (operator ruling, 2026-09-10, R0.1 §7 Q1): ANY stock
    /// edit — dimensions, alignment pins, or the material alone — makes
    /// every toolpath edited-since. Heights reference the stock top and an
    /// inherited boundary follows the stock outline, so geometry can move;
    /// the material moves the feeds. Both the GUI route and the MCP route
    /// reach this one function, so the two cannot disagree.
    pub(crate) fn drop_all_results(&mut self) {
        for index in 0..self.toolpath_configs.len() {
            self.drop_result(index);
        }
        self.simulation = None;
    }

    /// Drop the cached results of every toolpath in one setup, plus the
    /// simulation. The setup transform decides the frame every one of them
    /// is generated in.
    pub(crate) fn drop_setup_results(&mut self, setup_index: usize) {
        let Some(setup) = self.setups.get(setup_index) else {
            return;
        };
        let indices: Vec<usize> = setup.toolpath_indices.clone();
        for idx in indices {
            self.drop_result(idx);
        }
        self.simulation = None;
    }

    /// Public door onto the chain invalidation for callers that write
    /// `ToolpathConfig` fields directly rather than through a setter.
    ///
    /// The feeds Apply funnel is the one such caller
    /// (`controller/events/mod.rs::apply_feeds_through_funnel`, N13). It
    /// resolves the tool, the machine, the material and the
    /// recommendation from GUI state, then writes `operation` and
    /// `feeds_provenance` through `toolpath_configs_mut`, so no core
    /// setter runs. It reads
    /// [`ToolpathConfig::generation_inputs_signature`](super::ToolpathConfig::generation_inputs_signature)
    /// either side of that write and calls this door when the signature
    /// moved. Before this door existed the core kept the PREVIOUS result
    /// and export emitted it (R0.1 §2.2 item 1), and the downstream
    /// `FromRemainingStock` chain was never invalidated either.
    ///
    /// The GUI inspector was the other such caller until WP5. It takes
    /// [`Command::ReplaceToolpathConfig`](super::Command) now, which
    /// carries the same gate inside core. The funnel keeps this door
    /// until §15 ruling 4 folds `ApplyFeeds` into a row of its own.
    pub fn invalidate_toolpath_inputs(&mut self, index: usize) -> Effects {
        self.with_effects(Some(index), move |session| {
            let enabled = session
                .toolpath_configs
                .get(index)
                .is_some_and(|tc| tc.enabled);
            session.invalidate_result_chain(index, enabled);
        })
    }

    /// Set a setup's face-up orientation, dropping every result in that
    /// setup. The transform decides the frame the toolpaths are generated
    /// in, so none of them survives the change.
    ///
    /// Idempotent in the value: passing the face the setup already has
    /// still drops, because the GUI panel writes the field itself and then
    /// calls this to record the consequence.
    #[instrument(skip(self))]
    pub fn set_setup_face(
        &mut self,
        setup_index: usize,
        face_up: FaceUp,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.face_up = face_up;
            session.drop_setup_results(setup_index);
            Ok(())
        })
    }

    /// Set a setup's Z rotation, dropping every result in that setup.
    /// See [`Self::set_setup_face`].
    #[instrument(skip(self))]
    pub fn set_setup_rotation(
        &mut self,
        setup_index: usize,
        z_rotation: ZRotation,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            let setup = session
                .setups
                .get_mut(setup_index)
                .ok_or(SessionError::SetupNotFound(setup_index))?;
            setup.z_rotation = z_rotation;
            session.drop_setup_results(setup_index);
            Ok(())
        })
    }

    /// Wholesale replace all setups and toolpath configs from an external
    /// source (e.g. GUI's `JobState`).  This is the bulk-sync path used by
    /// `sync_session_from_job`.
    ///
    /// The caller is responsible for building valid `SetupData` and
    /// `ToolpathConfig` vecs whose `toolpath_indices` are consistent.
    #[instrument(skip(self, setups, toolpath_configs))]
    pub fn replace_setups_and_toolpaths(
        &mut self,
        setups: Vec<SetupData>,
        toolpath_configs: Vec<ToolpathConfig>,
    ) -> Effects {
        self.with_effects(None, move |session| {
            session.setups = setups;
            session.toolpath_configs = toolpath_configs;
            // Update the next-ID counters.
            session.next_toolpath_id = session
                .toolpath_configs
                .iter()
                .map(|tc| tc.id.0 + 1)
                .max()
                .unwrap_or(0);
            session.next_setup_id = session.setups.iter().map(|s| s.id + 1).max().unwrap_or(0);
            // Invalidate all cached results — the indices may have
            // shifted.
            session.results.clear();
            session.bump_all_revisions();
            session.simulation = None;
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::ids::ToolpathId;
    use crate::session::{AdoptResultArgs, Command, ReplaceToolpathConfigArgs};
    use std::sync::Arc;

    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::ToolpathStats;
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
    use crate::compute::operation_configs::{
        AlignmentPinDrillConfig, PencilConfig, PocketConfig, RestConfig,
    };
    use crate::compute::stock_config::FixtureId;
    use crate::debug_trace::ToolpathDebugOptions;
    use crate::gcode::CoolantMode;
    use crate::session::{Fixture, FixtureKind, KeepOutZone, ToolpathComputeResult};

    fn make_session() -> ProjectSession {
        ProjectSession::new_empty()
    }

    fn make_tc(tool_id: usize, model_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: ToolpathId(0),
            name: "test".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig::default()),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            planner_origin: None,
        }
    }

    fn make_tool() -> ToolConfig {
        ToolConfig::new_default(ToolId(0), crate::compute::tool_config::ToolType::EndMill)
    }

    fn fake_result() -> ToolpathComputeResult {
        ToolpathComputeResult {
            op_data: crate::drill_op::OpData::Toolpath(Arc::new(
                crate::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
            )),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
        }
    }

    // ── Toolpath CRUD ────────────────────────────────────────────

    #[test]
    fn add_toolpath_assigns_id_and_updates_setup() {
        let mut s = make_session();
        let tool_idx = s
            .add_tool(make_tool())
            .created
            .expect("add_tool reports the new tool index");
        assert_eq!(tool_idx, 0);

        let tc = make_tc(s.tools()[0].id.0, 0);
        let idx = s
            .add_toolpath(0, tc)
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        assert_eq!(idx, 0);
        assert_eq!(s.toolpath_configs()[0].id, ToolpathId(0));
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);

        let tc2 = make_tc(s.tools()[0].id.0, 0);
        let idx2 = s
            .add_toolpath(0, tc2)
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        assert_eq!(idx2, 1);
        assert_eq!(s.toolpath_configs()[1].id, ToolpathId(1));
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1]);
    }

    #[test]
    fn add_toolpath_invalid_setup() {
        let mut s = make_session();
        let tc = make_tc(0, 0);
        let result = s.add_toolpath(99, tc);
        assert!(matches!(result, Err(SessionError::SetupNotFound(99))));
    }

    #[test]
    fn add_toolpath_invalidates_simulation() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tc = make_tc(s.tools()[0].id.0, 0);
        let _ = s.add_toolpath(0, tc).unwrap();
        // simulation is cleared (set to None) by add_toolpath
        assert!(s.simulation.is_none());
    }

    #[test]
    fn remove_toolpath_shifts_indices() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;

        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1, 2]);

        // Add cached result for index 2
        s.results.insert(2, fake_result());

        let _ = s.remove_toolpath(0).unwrap();

        // Setup indices shifted: [1, 2] → [0, 1]
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1]);
        // Result for old index 2 should now be at index 1
        assert!(s.results.contains_key(&1));
        assert!(!s.results.contains_key(&2));
    }

    #[test]
    fn remove_toolpath_not_found() {
        let mut s = make_session();
        assert!(matches!(
            s.remove_toolpath(0),
            Err(SessionError::ToolpathNotFound(0))
        ));
    }

    #[test]
    fn reorder_toolpath_swaps() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;

        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();

        let id_0 = s.toolpath_configs()[0].id;
        let id_1 = s.toolpath_configs()[1].id;

        let _ = s.reorder_toolpath(0, 1).unwrap();
        // Global config order is stable; only the setup's display order swaps.
        assert_eq!(s.toolpath_configs()[0].id, id_0);
        assert_eq!(s.toolpath_configs()[1].id, id_1);
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![1, 0]);
    }

    #[test]
    fn set_toolpath_enabled_invalidates_sim() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        let _ = s.set_toolpath_enabled(0, false).unwrap();
        assert!(!s.toolpath_configs()[0].enabled);
        assert!(s.simulation.is_none());
    }

    #[test]
    fn set_dressup_invalidates_result_and_sim() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let _ = s.set_dressup_config(0, DressupConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn set_heights_invalidates_result_and_sim() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let _ = s.set_heights_config(0, HeightsConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    // ── Staleness-chain invalidation sentries ────────────────────
    //
    // 2026-07-09 live-v2 collision class: a FromRemainingStock finish op was
    // generated while an upstream finish op was enabled; the user disabled
    // the upstream op and the KEPT downstream result — whose
    // optimize_entry_descents rapids were lowered against the old, deeper
    // prior stock — grazed the now-taller stock at rapid feed (20 rapid
    // collisions at z = old_ceiling + 2 mm). Chain edits must kill dependent
    // cached results (`invalidate_result_chain`).

    #[test]
    fn toggle_enabled_invalidates_downstream_rest_results() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        let mut rest = make_tc(tool_id, 0);
        rest.stock_source = crate::session::StockSource::FromRemainingStock;
        let _ = s.add_toolpath(0, rest).unwrap();
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.results.insert(0, fake_result());
        s.results.insert(1, fake_result());
        s.results.insert(2, fake_result());

        let _ = s.set_toolpath_enabled(0, false).unwrap();
        // The toggled op keeps its own result (still valid on re-enable)…
        assert!(s.results.contains_key(&0));
        // …the downstream FromRemainingStock result dies (generated against
        // a stock chain that no longer exists)…
        assert!(!s.results.contains_key(&1));
        // …and a downstream Fresh-stock op is untouched.
        assert!(s.results.contains_key(&2));

        // Toggling back is ALSO a chain change (cuts reappear upstream).
        s.results.insert(1, fake_result());
        let _ = s.set_toolpath_enabled(0, true).unwrap();
        assert!(!s.results.contains_key(&1));
    }

    #[test]
    fn content_edit_invalidates_rest_dependents_transitively() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        let mut rest = make_tc(tool_id, 0);
        rest.stock_source = crate::session::StockSource::FromRemainingStock;
        let _ = s.add_toolpath(0, rest).unwrap();
        // Consumer of op1's derived rest regions (Fresh stock — reached only
        // through the DerivedRestRegions edge, not the stock chain).
        let mut consumer = make_tc(tool_id, 0);
        consumer.boundary = BoundaryConfig {
            enabled: true,
            source: crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id: ToolpathId(1),
            },
            ..BoundaryConfig::default()
        };
        let _ = s.add_toolpath(0, consumer).unwrap();
        s.results.insert(0, fake_result());
        s.results.insert(1, fake_result());
        s.results.insert(2, fake_result());

        let _ = s.set_heights_config(0, HeightsConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0), "edited op invalidated");
        assert!(
            !s.results.contains_key(&1),
            "downstream rest op invalidated via the stock chain"
        );
        assert!(
            !s.results.contains_key(&2),
            "rest-region consumer invalidated transitively"
        );
    }

    #[test]
    fn disabled_op_edit_leaves_downstream_alone() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        let mut off = make_tc(tool_id, 0);
        off.enabled = false;
        let _ = s.add_toolpath(0, off).unwrap();
        let mut rest = make_tc(tool_id, 0);
        rest.stock_source = crate::session::StockSource::FromRemainingStock;
        let _ = s.add_toolpath(0, rest).unwrap();
        s.results.insert(1, fake_result());

        // Editing an op that is (and stays) disabled doesn't change the
        // material-removal chain — downstream results survive.
        let _ = s.set_heights_config(0, HeightsConfig::default()).unwrap();
        assert!(s.results.contains_key(&1));
    }

    #[test]
    fn set_boundary_invalidates_result_and_sim() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let _ = s.set_boundary_config(0, BoundaryConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn set_boundary_derived_rest_regions_auto_enables_source_rest_analysis() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        // Source toolpath (a plain pocket): rest analysis starts disabled.
        let source_idx = s
            .add_toolpath(0, make_tc(tool_id, 0))
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        let source_id = s.toolpath_configs()[source_idx].id;
        // Consumer toolpath.
        let consumer_idx = s
            .add_toolpath(0, make_tc(tool_id, 0))
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        s.results.insert(source_idx, fake_result());

        let boundary = BoundaryConfig {
            enabled: true,
            source: crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id: source_id,
            },
            ..BoundaryConfig::default()
        };
        let _ = s.set_boundary_config(consumer_idx, boundary).unwrap();

        assert!(s.toolpath_configs()[source_idx].rest_analysis.enabled);
        // The source's own cached result must be invalidated — it needs to
        // regenerate to actually attach the rest regions.
        assert!(!s.results.contains_key(&source_idx));
    }

    #[test]
    fn set_boundary_derived_rest_regions_skips_rest_depth_pencil_source() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;

        let source_tc = ToolpathConfig {
            operation: OperationConfig::Pencil(PencilConfig {
                detector: "rest_depth".to_owned(),
                ..PencilConfig::default()
            }),
            ..make_tc(tool_id, 0)
        };
        let source_idx = s
            .add_toolpath(0, source_tc)
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        let source_id = s.toolpath_configs()[source_idx].id;
        let consumer_idx = s
            .add_toolpath(0, make_tc(tool_id, 0))
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");

        let boundary = BoundaryConfig {
            enabled: true,
            source: crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id: source_id,
            },
            ..BoundaryConfig::default()
        };
        let _ = s.set_boundary_config(consumer_idx, boundary).unwrap();

        // A rest_depth pencil already attaches its own rest artifacts —
        // forcing the generic flag on would be redundant, so it stays off.
        assert!(!s.toolpath_configs()[source_idx].rest_analysis.enabled);
    }

    #[test]
    fn rest_region_consumers_finds_enabled_consumers_only() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;

        let source_idx = s
            .add_toolpath(0, make_tc(tool_id, 0))
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        let source_id = s.toolpath_configs()[source_idx].id;
        let enabled_consumer_idx = s
            .add_toolpath(0, make_tc(tool_id, 0))
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        let disabled_consumer_idx = s
            .add_toolpath(0, make_tc(tool_id, 0))
            .unwrap()
            .created
            .expect("add_toolpath reports the new toolpath index");
        let enabled_consumer_id = s.toolpath_configs()[enabled_consumer_idx].id;

        let enabled_boundary = BoundaryConfig {
            enabled: true,
            source: crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id: source_id,
            },
            ..BoundaryConfig::default()
        };
        let _ = s
            .set_boundary_config(enabled_consumer_idx, enabled_boundary.clone())
            .unwrap();
        let disabled_boundary = BoundaryConfig {
            enabled: false,
            ..enabled_boundary
        };
        let _ = s
            .set_boundary_config(disabled_consumer_idx, disabled_boundary)
            .unwrap();

        assert_eq!(
            s.rest_region_consumers(source_id),
            vec![enabled_consumer_id]
        );
    }

    // ── Tool CRUD ────────────────────────────────────────────────

    #[test]
    fn add_tool_returns_index_and_assigns_id() {
        let mut s = make_session();
        let idx = s
            .add_tool(make_tool())
            .created
            .expect("add_tool reports the new tool index");
        assert_eq!(idx, 0);
        let idx2 = s
            .add_tool(make_tool())
            .created
            .expect("add_tool reports the new tool index");
        assert_eq!(idx2, 1);
        assert_ne!(s.tools()[0].id, s.tools()[1].id);
    }

    #[test]
    fn remove_tool_in_use_errors() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();

        let result = s.remove_tool(0);
        assert!(matches!(result, Err(SessionError::ToolInUse(_))));
    }

    #[test]
    fn remove_tool_not_in_use_succeeds() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_tool(make_tool());
        assert_eq!(s.tools().len(), 2);

        let _ = s.remove_tool(1).unwrap();
        assert_eq!(s.tools().len(), 1);
    }

    // ── Setup CRUD ───────────────────────────────────────────────

    #[test]
    fn add_setup_returns_index() {
        let mut s = make_session();
        // new_empty already creates setup 0
        assert_eq!(s.list_setups().len(), 1);

        let idx = s
            .add_setup("Setup 2".to_owned(), FaceUp::Bottom)
            .created
            .expect("add_setup reports the new setup index");
        assert_eq!(idx, 1);
        assert_eq!(s.list_setups().len(), 2);
        assert_eq!(s.list_setups()[1].name, "Setup 2");
        assert_eq!(s.list_setups()[1].face_up, FaceUp::Bottom);
    }

    #[test]
    fn remove_setup_with_toolpaths_errors() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        let result = s.remove_setup(0);
        assert!(matches!(result, Err(SessionError::SetupHasToolpaths(0))));
    }

    #[test]
    fn remove_empty_setup_succeeds() {
        let mut s = make_session();
        let _ = s.add_setup("Extra".to_owned(), FaceUp::default());
        assert_eq!(s.list_setups().len(), 2);

        let _ = s.remove_setup(1).unwrap();
        assert_eq!(s.list_setups().len(), 1);
    }

    // ── Cross-setup move ─────────────────────────────────────────

    #[test]
    fn move_toolpath_to_setup_moves_and_invalidates() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_setup("Setup 2".to_owned(), FaceUp::Bottom);

        let tool_id = s.tools()[0].id.0;
        let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.results.insert(0, fake_result());

        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);
        assert!(s.list_setups()[1].toolpath_indices.is_empty());

        let _ = s.move_toolpath_to_setup(0, 1, None).unwrap();

        assert!(s.list_setups()[0].toolpath_indices.is_empty());
        assert_eq!(s.list_setups()[1].toolpath_indices, vec![0]);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn move_toolpath_invalid_target() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        assert!(matches!(
            s.move_toolpath_to_setup(0, 99, None),
            Err(SessionError::SetupNotFound(99))
        ));
    }

    // ── Face selection ───────────────────────────────────────────

    #[test]
    fn set_face_selection_invalidates() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let faces = vec![
            crate::enriched_mesh::FaceGroupId(1),
            crate::enriched_mesh::FaceGroupId(3),
        ];
        let _ = s.set_face_selection(0, Some(faces)).unwrap();

        assert!(s.toolpath_configs()[0].face_selection.is_some());
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    // ── Rename setup ─────────────────────────────────────────────

    #[test]
    fn rename_setup_changes_name() {
        let mut s = make_session();
        let _ = s.rename_setup(0, "New Name".to_owned()).unwrap();
        assert_eq!(s.list_setups()[0].name, "New Name");
    }

    #[test]
    fn rename_setup_not_found() {
        let mut s = make_session();
        assert!(matches!(
            s.rename_setup(99, "x".to_owned()),
            Err(SessionError::SetupNotFound(99))
        ));
    }

    // ── Fixture CRUD ─────────────────────────────────────────────

    #[test]
    fn add_fixture_invalidates_setup_toolpaths() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let fixture = Fixture {
            id: FixtureId(0),
            name: "Clamp 1".to_owned(),
            kind: FixtureKind::Clamp,
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            size_x: 30.0,
            size_y: 15.0,
            size_z: 20.0,
            clearance: 3.0,
        };
        let _ = s.add_fixture(0, fixture).unwrap();

        assert_eq!(s.list_setups()[0].fixtures.len(), 1);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn remove_fixture_by_id() {
        let mut s = make_session();
        let fixture = Fixture {
            id: FixtureId(42),
            name: "Clamp".to_owned(),
            kind: FixtureKind::Clamp,
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            size_x: 10.0,
            size_y: 10.0,
            size_z: 10.0,
            clearance: 1.0,
        };
        let _ = s.add_fixture(0, fixture).unwrap();
        assert_eq!(s.list_setups()[0].fixtures.len(), 1);

        let _ = s.remove_fixture(0, FixtureId(42)).unwrap();
        assert!(s.list_setups()[0].fixtures.is_empty());
    }

    // ── Keep-out CRUD ────────────────────────────────────────────

    #[test]
    fn add_keep_out_invalidates_setup_toolpaths() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let zone = KeepOutZone {
            id: crate::compute::stock_config::KeepOutId(0),
            name: "Zone 1".to_owned(),
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            size_x: 20.0,
            size_y: 20.0,
        };
        let _ = s.add_keep_out(0, zone).unwrap();

        assert_eq!(s.list_setups()[0].keep_out_zones.len(), 1);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn remove_keep_out_by_id() {
        let mut s = make_session();
        let zone = KeepOutZone {
            id: crate::compute::stock_config::KeepOutId(7),
            name: "Zone".to_owned(),
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            size_x: 10.0,
            size_y: 10.0,
        };
        let _ = s.add_keep_out(0, zone).unwrap();
        assert_eq!(s.list_setups()[0].keep_out_zones.len(), 1);

        let _ = s
            .remove_keep_out(0, crate::compute::stock_config::KeepOutId(7))
            .unwrap();
        assert!(s.list_setups()[0].keep_out_zones.is_empty());
    }

    // ── Alignment pin drill ──────────────────────────────────────

    #[test]
    fn set_alignment_pin_drill_holes() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());

        let tc = ToolpathConfig {
            operation: OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
            ..make_tc(s.tools()[0].id.0, 0)
        };
        let _ = s.add_toolpath(0, tc).unwrap();
        s.results.insert(0, fake_result());

        let holes = vec![[10.0, 20.0], [90.0, 20.0]];
        let _ = s.set_alignment_pin_drill_holes(0, holes).unwrap();

        match &s.toolpath_configs()[0].operation {
            OperationConfig::AlignmentPinDrill(cfg) => {
                assert_eq!(cfg.holes.len(), 2);
                assert_eq!(cfg.holes[0], [10.0, 20.0]);
            }
            _ => panic!("Expected AlignmentPinDrill"),
        }
        assert!(!s.results.contains_key(&0));
    }

    #[test]
    fn set_alignment_pin_drill_holes_wrong_op() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        let result = s.set_alignment_pin_drill_holes(0, vec![]);
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    // ── Invalidation helpers ─────────────────────────────────────

    #[test]
    fn invalidate_stock_clears_simulation() {
        let mut s = make_session();
        // simulation starts as None; invalidate should keep it None
        let _ = s.invalidate_stock();
        assert!(s.simulation.is_none());
    }

    #[test]
    fn invalidate_machine_clears_simulation() {
        let mut s = make_session();
        let _ = s.invalidate_machine();
        assert!(s.simulation.is_none());
    }

    #[test]
    fn invalidate_tool_clears_matching_results() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_tool(make_tool());
        let tool_id_0 = s.tools()[0].id.0;
        let tool_id_1 = s.tools()[1].id.0;

        let _ = s.add_toolpath(0, make_tc(tool_id_0, 0)).unwrap(); // idx 0
        let _ = s.add_toolpath(0, make_tc(tool_id_1, 0)).unwrap(); // idx 1
        let _ = s.add_toolpath(0, make_tc(tool_id_0, 0)).unwrap(); // idx 2

        s.results.insert(0, fake_result());
        s.results.insert(1, fake_result());
        s.results.insert(2, fake_result());

        let _ = s.invalidate_tool(tool_id_0);

        // Results for toolpaths using tool_id_0 (idx 0, 2) should be cleared
        assert!(!s.results.contains_key(&0));
        assert!(s.results.contains_key(&1)); // uses tool_id_1, unaffected
        assert!(!s.results.contains_key(&2));
    }

    // ── Global config ────────────────────────────────────────────

    #[test]
    fn set_stock_config_runs() {
        let mut s = make_session();
        let _ = s.set_stock_config(StockConfig::default());
        assert!(s.simulation.is_none());
    }

    #[test]
    fn replace_tools_updates_id_counter() {
        let mut s = make_session();
        let mut t1 = make_tool();
        t1.id = ToolId(5);
        let mut t2 = make_tool();
        t2.id = ToolId(10);
        let _ = s.replace_tools(vec![t1, t2]);

        // next_tool_id should be max(5, 10) + 1 = 11
        let new_idx = s
            .add_tool(make_tool())
            .created
            .expect("add_tool reports the new tool index");
        assert_eq!(s.tools()[new_idx].id, ToolId(11));
    }

    /// Replace the configuration at index 0 through the command door.
    fn replace(s: &mut ProjectSession, config: ToolpathConfig) -> Effects {
        s.apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(config),
        }))
        .expect("index 0 exists")
    }

    /// WP5. A replacement that moves no generation input keeps the
    /// cached result, and the four fields outside the signature land.
    ///
    /// The GUI inspector applies this command on every frame the panel is
    /// open, because it holds no commit event. An ungated replacement —
    /// which is what `replace_toolpath_config` was before WP5 — would
    /// therefore drop the geometry on every such frame.
    #[test]
    fn replace_toolpath_config_outside_the_signature_keeps_the_result() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let mut edited = s.toolpath_configs()[0].clone();
        edited.name = "renamed".to_owned();
        edited.coolant = CoolantMode::Flood;
        edited.pre_gcode = Some("M3 S18000".to_owned());
        edited.post_gcode = Some("M5".to_owned());

        let effects = replace(&mut s, edited);

        assert!(
            effects.stale.is_empty(),
            "name, coolant and the pre and post G-code change no motion"
        );
        assert!(
            s.results.contains_key(&0),
            "the cached geometry still answers every input that decides it"
        );
        let stored = &s.toolpath_configs()[0];
        assert_eq!(stored.name, "renamed");
        assert_eq!(stored.coolant, CoolantMode::Flood);
        assert_eq!(stored.pre_gcode.as_deref(), Some("M3 S18000"));
        assert_eq!(stored.post_gcode.as_deref(), Some("M5"));
    }

    /// WP5. A replacement that moves a generation input drops the result.
    #[test]
    fn replace_toolpath_config_on_a_moved_signature_invalidates() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let mut edited = s.toolpath_configs()[0].clone();
        edited.operation.set_feed_rate(4321.0);

        let effects = replace(&mut s, edited);

        assert_eq!(effects.stale, BTreeSet::from([0_usize]));
        assert!(!s.results.contains_key(&0));
    }

    #[test]
    fn update_stock_from_bbox_invalidates_sim() {
        let mut s = make_session();
        let pad = s.stock_config().padding;
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(120.0, 80.0, 25.0),
        };
        let _ = s.update_stock_from_bbox(&bbox);

        // Stock grew to fit bbox + padding (XY) and exact bbox + padding (Z).
        assert!((s.stock_config().x - (120.0 + 2.0 * pad)).abs() < 1e-6);
        assert!((s.stock_config().y - (80.0 + 2.0 * pad)).abs() < 1e-6);
        assert!((s.stock_config().z - (25.0 + pad)).abs() < 1e-6);
        // Simulation cache is cleared (mirrors the pattern in
        // set_toolpath_enabled_invalidates_sim).
        assert!(s.simulation.is_none());
    }

    #[test]
    fn apply_toolpath_param_snapshot_narrow_invalidates() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        // Snapshot of "prior" state we want to restore.
        let snapshot_op = OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default());
        let snapshot_dress = DressupConfig::default();
        let snapshot_faces = Some(vec![crate::enriched_mesh::FaceGroupId(7)]);

        let _ = s
            .apply_toolpath_param_snapshot_narrow(
                0,
                snapshot_op,
                snapshot_dress,
                snapshot_faces.clone(),
            )
            .unwrap();

        match &s.toolpath_configs()[0].operation {
            OperationConfig::AlignmentPinDrill(_) => {}
            _ => panic!("operation snapshot not applied"),
        }
        assert_eq!(s.toolpath_configs()[0].face_selection, snapshot_faces);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn apply_toolpath_param_snapshot_narrow_not_found() {
        let mut s = make_session();
        let result = s.apply_toolpath_param_snapshot_narrow(
            99,
            OperationConfig::Pocket(PocketConfig::default()),
            DressupConfig::default(),
            None,
        );
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
    }

    /// The optimizer's dependence on the narrowness, pinned.
    ///
    /// `tool_load/optimize/candidate.rs` restores a candidate, then
    /// regenerates ONE index, then simulates against the neighbours'
    /// cached results. The narrow path is what leaves those results in
    /// place. `Command::RestoreToolpathSnapshot` drops them, which is
    /// the right answer for an undo and the wrong answer for one point
    /// of a candidate search.
    ///
    /// The wide half of this pair lives in
    /// `tests/restore_snapshot_invalidates_like_the_setter_n14.rs`. It
    /// cannot reach this path, because `pub(crate)` keeps the path
    /// inside the crate.
    #[test]
    fn the_narrow_path_leaves_the_neighbour_result_cached() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let tool = s.tools()[0].id.0;
        let _ = s.add_toolpath(0, make_tc(tool, 0)).unwrap();
        let mut downstream = make_tc(tool, 0);
        downstream.operation = OperationConfig::Rest(RestConfig::default());
        downstream.stock_source = crate::session::StockSource::FromRemainingStock;
        let _ = s.add_toolpath(0, downstream).unwrap();
        s.results.insert(0, fake_result());
        s.results.insert(1, fake_result());
        assert!(
            s.get_result(0).is_some() && s.get_result(1).is_some(),
            "both rows need a cached result, or the reading below is vacuous"
        );

        let _ = s
            .apply_toolpath_param_snapshot_narrow(
                0,
                OperationConfig::Pocket(PocketConfig::default()),
                DressupConfig::default(),
                None,
            )
            .unwrap();

        assert!(
            s.get_result(0).is_none(),
            "the restored row loses its own result on either contract"
        );
        assert!(
            s.get_result(1).is_some(),
            "and the row that reads its remaining stock keeps its \
             result. The candidate search regenerates one index and \
             simulates against this cache."
        );
    }

    /// Deliver a completion the way the compute lane does.
    ///
    /// `insert_result` is no longer a public door. A completion arrives
    /// as `Command::AdoptResult`, which carries the revision the lane
    /// started from.
    fn adopt(s: &mut ProjectSession, index: usize) {
        let revision = s.toolpath_revision(index);
        let _ = s
            .apply(Command::AdoptResult(AdoptResultArgs {
                index,
                revision,
                result: Box::new(fake_result()),
            }))
            .expect("the fixture adopts at the current revision");
    }

    #[test]
    fn adopt_result_populates_cache() {
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        assert!(s.get_result(0).is_none());
        adopt(&mut s, 0);
        assert!(s.get_result(0).is_some());
    }

    #[test]
    fn adopt_result_after_apply_snapshot_restores_cache() {
        // Roadmap F.1: the snapshot path clears results[idx].
        // The viz-side regen used to leave that empty
        // because writes only landed in gui.toolpath_rt. The adoption
        // door is the symmetric write that closes the cache gap.
        let mut s = make_session();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let snapshot = s.apply_toolpath_param_snapshot_narrow(
            0,
            OperationConfig::Pocket(PocketConfig::default()),
            DressupConfig::default(),
            None,
        );
        assert!(snapshot.is_ok(), "index 0 exists");
        assert!(s.get_result(0).is_none());

        adopt(&mut s, 0);
        assert!(s.get_result(0).is_some());
    }

    #[test]
    fn adopt_result_rejects_out_of_range_index() {
        let mut s = make_session();
        let err = s
            .apply(Command::AdoptResult(AdoptResultArgs {
                index: 99,
                revision: 0,
                result: Box::new(fake_result()),
            }))
            .unwrap_err();
        assert!(matches!(err, SessionError::ToolpathNotFound(99)));
    }
}
