//! Bounded-response primitives for MCP reads (TD3 wave B-2, Checkpoint L).
//!
//! # Why this exists
//!
//! Measured 2026-08-08 on a wanaka-scale trace (1,588,883 samples, cell
//! 0.1 mm) by wave B-1: one unfiltered `get_cut_trace` emitted a
//! **60,517,035-byte** single JSON-RPC line (56,225,225 bytes of tool text).
//! That is **16,245×** the median MCP response in the same census (3,461 B)
//! and **710×** the largest response of any other tool (79,135 B). The
//! server survived serialising it — the OOM hypothesis is falsified
//! (`planning/review_2026-08-08/GLV2_CRASH_CAPTURE.md` §2) — but the frame
//! never reached the agent that asked for it.
//!
//! **94 %** of that payload was one uncapped array, `span_summaries`, and
//! **92 %** of it came from one operation. Five arrays on that response had
//! no cap and no way to say they had been shortened.
//!
//! Checkpoint L (ruled 2026-08-08, Option 2) prescribes two layers:
//!
//! 1. **Per-array caps** carrying the response's *existing* truncation
//!    vocabulary — `<name>_total_matching`, `<name>_returned`,
//!    `<name>_truncated` — so a shortened array always says so.
//! 2. A **global byte backstop** ([`MAX_RESPONSE_BYTES`]) charged **while
//!    the response is built**, not by truncating a finished string. On
//!    overflow the response still completes with the sections that fit,
//!    names the ones that did not under `sections_not_computed`, and
//!    carries `complete: false`.
//!
//! # The one rule that is not negotiable
//!
//! C25's standing rule: **"did not fit" is never rendered as a zero.** A
//! section that overflowed the budget is **omitted**, not emitted as `[]` or
//! `0`, and its name appears in `sections_not_computed`. An agent that reads
//! a missing key cannot mistake it for a measurement; an agent that reads
//! `[]` can, and would.
//!
//! Continuation tokens are deliberately **not** built here (Checkpoint L
//! deferred them): the census found no read that needs pagination — the unit
//! an agent actually wants is "one toolpath" or "one span kind", and both are
//! already filters.

use serde_json::{Map, Value};

/// Global response-size backstop, in bytes (Checkpoint L-3: 8 MiB).
///
/// Sized from B-1's census rather than as a round number: **3.95×** the
/// largest response any *useful* filtered call produced (2,122,146 B),
/// **106×** the largest non-`get_cut_trace` response (79,135 B), and
/// **7.2×** below the 60,517,035-byte frame a real client did not survive.
pub const MAX_RESPONSE_BYTES: usize = 8_388_608;

/// Default cap on `get_cut_trace`'s `span_summaries` (Checkpoint L-3).
///
/// 200 keeps a whole-project response in the same order as the largest
/// *bounded* read that ships today (`get_generation_debug_trace` at its
/// 100-span default, 32.6 kB on the census fixture). The uncapped array
/// reached **35,838** entries on that same fixture.
pub const DEFAULT_MAX_SPAN_SUMMARIES: usize = 200;

/// Default cap on `get_cut_trace`'s `semantic_summaries` (Checkpoint L-3).
///
/// The array arrives sorted by `wasted_runtime_s` descending
/// (`rs_cam_core::simulation_cut`), so the first 200 are the interesting
/// ones. That ordering is a documented part of the cap contract — see
/// [`ORDERING_SEMANTIC_SUMMARIES`].
pub const DEFAULT_MAX_SEMANTIC_SUMMARIES: usize = 200;

/// Default cap on `get_cut_trace`'s opt-in `drill_samples` (Checkpoint L-3).
pub const DEFAULT_MAX_DRILL_SAMPLES: usize = 500;

/// Default cap on `inspect_spans`' summary-mode `top_level` array
/// (Checkpoint L-3, "same cap in the same commit for uniformity").
///
/// **This is a latent bound, not a measured cost.** On B-1's 33,195-span
/// Unified Finish, summary mode returned **685 bytes** — `top_level` holds
/// only `Operation` and `DepthPass` spans, of which that fixture has few.
/// Nothing here fixed an observed problem; it removes an unbounded array.
pub const DEFAULT_MAX_TOP_LEVEL_SPANS: usize = 200;

/// Documented ordering contract for `span_summaries` under a cap.
///
/// Toolpath order is the project's own toolpath index ascending; within a
/// toolpath, span id (the span vector index) ascending. Deterministic and
/// stable across calls on an unchanged project, so `max_span_summaries` and
/// a span filter compose predictably.
pub const ORDERING_SPAN_SUMMARIES: &str =
    "toolpath index ascending, then span_id ascending (span vector order)";

/// Documented ordering contract for `semantic_summaries` under a cap.
///
/// Produced sorted by `wasted_runtime_s` descending by
/// `rs_cam_core::simulation_cut`; filtering preserves that order, so the
/// first N under a cap are the N worst offenders.
pub const ORDERING_SEMANTIC_SUMMARIES: &str = "wasted_runtime_s descending";

/// A byte budget charged while a response is being assembled.
///
/// The budget is charged with the **compact** serialised length of each
/// value, which is what [`crate::server::json_str`] emits. It tracks the
/// sections that did not fit so the finished response can name them instead
/// of silently dropping them.
#[derive(Debug, Clone)]
pub struct ResponseBudget {
    limit: usize,
    used: usize,
    not_computed: Vec<String>,
}

impl ResponseBudget {
    /// A budget with an explicit byte limit.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            not_computed: Vec::new(),
        }
    }

    /// An effectively unbounded budget.
    ///
    /// Exists so a sentry can reconstruct the **pre-fix** behaviour on the
    /// shipped code path — the 60 MB response is reproducible by asking for
    /// it explicitly, which keeps the red state executable rather than
    /// remembered.
    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(usize::MAX)
    }

    #[must_use]
    pub fn limit(&self) -> usize {
        self.limit
    }

    #[must_use]
    pub fn used(&self) -> usize {
        self.used
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.used)
    }

    /// Compact serialised length of `value`.
    ///
    /// A value that cannot be serialised is reported as [`usize::MAX`], so
    /// it can never be charged and can never fit — it will be named in
    /// `sections_not_computed` rather than emitted as an empty stand-in.
    #[must_use]
    pub fn cost_of(value: &Value) -> usize {
        serde_json::to_string(value).map_or(usize::MAX, |s| s.len())
    }

    /// Would `bytes` more fit?
    #[must_use]
    pub fn would_fit(&self, bytes: usize) -> bool {
        bytes != usize::MAX && self.used.saturating_add(bytes) <= self.limit
    }

    /// Charge `bytes` unconditionally (used for keys the response must
    /// always carry, such as scalars and the truncation vocabulary itself).
    pub fn charge(&mut self, bytes: usize) {
        self.used = self.used.saturating_add(bytes);
    }

    /// Charge `value` if it fits. Charges **nothing** and returns `false`
    /// when it does not — the caller must then omit the section and record
    /// it via [`Self::record_not_computed`].
    pub fn try_charge(&mut self, value: &Value) -> bool {
        let cost = Self::cost_of(value);
        if self.would_fit(cost) {
            self.charge(cost);
            true
        } else {
            false
        }
    }

    /// Record a section that was left out because it did not fit.
    pub fn record_not_computed(&mut self, section: &str) {
        let owned = section.to_owned();
        if !self.not_computed.contains(&owned) {
            self.not_computed.push(owned);
        }
    }

    /// Sections omitted for want of budget, in the order they were tried.
    #[must_use]
    pub fn sections_not_computed(&self) -> &[String] {
        &self.not_computed
    }

    /// True when every section the caller offered was included.
    ///
    /// Note this says nothing about *truncation*: an array shortened by its
    /// own item cap is still a complete response. Truncation is reported per
    /// array by the `_truncated` keys; `complete` reports only whether a
    /// whole section had to be dropped.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.not_computed.is_empty()
    }
}

/// An array shortened by an item cap and/or the byte budget, together with
/// the true pre-cap population size.
#[derive(Debug, Clone)]
pub struct CappedArray {
    values: Vec<Value>,
    total_matching: usize,
    cap: usize,
}

impl CappedArray {
    /// Number of entries actually in the response.
    #[must_use]
    pub fn returned(&self) -> usize {
        self.values.len()
    }

    /// True pre-cap count of entries matching the request's filters.
    #[must_use]
    pub fn total_matching(&self) -> usize {
        self.total_matching
    }

    /// Whether anything was left out — for **either** reason (item cap or
    /// byte budget). A caller comparing `returned` against `total_matching`
    /// gets the same answer.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.returned() < self.total_matching
    }

    /// The cap that was in force, so an agent can raise it deliberately.
    #[must_use]
    pub fn cap(&self) -> usize {
        self.cap
    }

    #[must_use]
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// The array itself.
    #[must_use]
    pub fn into_value(self) -> Value {
        Value::Array(self.values)
    }

    /// Emit the truncation vocabulary under `prefix`.
    ///
    /// Keys: `<prefix>_total_matching`, `<prefix>_returned`,
    /// `<prefix>_truncated`, `<prefix>_cap`. The first three are exactly the
    /// triple `hotspots` and `issues` have carried since R-1; `_cap` is
    /// additive and names the bound that was applied.
    pub fn insert_keys(&self, prefix: &str, map: &mut Map<String, Value>) {
        map.insert(
            format!("{prefix}_total_matching"),
            Value::from(self.total_matching),
        );
        map.insert(format!("{prefix}_returned"), Value::from(self.returned()));
        map.insert(format!("{prefix}_truncated"), Value::Bool(self.truncated()));
        map.insert(
            format!("{prefix}_cap"),
            if self.cap == usize::MAX {
                Value::Null
            } else {
                Value::from(self.cap)
            },
        );
    }
}

/// Take up to `cap` values from `items`, stopping early if the byte budget
/// runs out, and charge what was taken.
///
/// `total_matching` is the caller's own **pre-cap** count and is reported
/// verbatim — it is never inferred from what was emitted, so an agent can
/// always tell how much it did not receive.
///
/// `items` is consumed lazily: with a lazy iterator nothing beyond the cap is
/// ever built, which is what makes the bound apply *while building* rather
/// than to a finished string.
pub fn cap_json_values<I>(
    total_matching: usize,
    items: I,
    cap: usize,
    budget: &mut ResponseBudget,
) -> CappedArray
where
    I: IntoIterator<Item = Value>,
{
    // The enclosing `[]` plus one separator per entry after the first.
    budget.charge(2);
    let mut values: Vec<Value> = Vec::new();
    for value in items {
        if values.len() >= cap {
            break;
        }
        let cost = ResponseBudget::cost_of(&value).saturating_add(1);
        if !budget.would_fit(cost) {
            break;
        }
        budget.charge(cost);
        values.push(value);
    }
    CappedArray {
        values,
        total_matching,
        cap,
    }
}

/// A JSON object assembled under a byte budget.
///
/// Insert the small, load-bearing keys first and the large arrays last: on
/// overflow the sections offered last are the ones dropped, so section order
/// **is** the priority order.
#[derive(Debug)]
pub struct BoundedResponse {
    map: Map<String, Value>,
    budget: ResponseBudget,
}

impl BoundedResponse {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            map: Map::new(),
            budget: ResponseBudget::new(limit),
        }
    }

    #[must_use]
    pub fn budget(&self) -> &ResponseBudget {
        &self.budget
    }

    pub fn budget_mut(&mut self) -> &mut ResponseBudget {
        &mut self.budget
    }

    /// A key the response must always carry (scalars, counts, vocabulary).
    /// Charged against the budget but never dropped — dropping a scalar
    /// would save nothing and cost the reader its bearings.
    pub fn insert_always(&mut self, key: &str, value: Value) {
        self.budget.charge(ResponseBudget::cost_of(&value));
        self.map.insert(key.to_owned(), value);
    }

    /// An optional section. Included iff it fits; otherwise **omitted**
    /// (never `[]`, never `0`) and named in `sections_not_computed`.
    ///
    /// Returns whether it was included.
    pub fn insert_section(&mut self, key: &str, value: Value) -> bool {
        if self.budget.try_charge(&value) {
            self.map.insert(key.to_owned(), value);
            true
        } else {
            self.budget.record_not_computed(key);
            false
        }
    }

    /// A capped array section plus its truncation vocabulary.
    ///
    /// The entries were charged as they were taken by [`cap_json_values`],
    /// so this always succeeds; what did not fit is already reported by
    /// `<key>_truncated` / `<key>_returned` against a true
    /// `<key>_total_matching`.
    pub fn insert_capped(&mut self, key: &str, array: CappedArray) {
        array.insert_keys(key, &mut self.map);
        self.map.insert(key.to_owned(), array.into_value());
    }

    /// Finish the object, appending the completeness vocabulary.
    ///
    /// `complete` is `false` exactly when at least one section was dropped;
    /// `sections_not_computed` names them. `max_response_bytes` and
    /// `response_bytes_estimated` let a caller see how close it came.
    #[must_use]
    pub fn finish(mut self) -> Value {
        let complete = self.budget.complete();
        let not_computed: Vec<Value> = self
            .budget
            .sections_not_computed()
            .iter()
            .map(|s| Value::from(s.clone()))
            .collect();
        self.map.insert("complete".to_owned(), Value::Bool(complete));
        self.map.insert(
            "sections_not_computed".to_owned(),
            Value::Array(not_computed),
        );
        self.map.insert(
            "max_response_bytes".to_owned(),
            if self.budget.limit() == usize::MAX {
                Value::Null
            } else {
                Value::from(self.budget.limit())
            },
        );
        self.map.insert(
            "response_bytes_estimated".to_owned(),
            Value::from(self.budget.used()),
        );
        Value::Object(self.map)
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

    /// One entry shaped like a real `span_summaries` row. B-1 measured the
    /// shipped rows at **1,475 bytes each** as served (pretty); this stand-in
    /// is the same order of magnitude compact, which is what the budget
    /// charges.
    fn span_summary_row(i: usize) -> Value {
        serde_json::json!({
            "toolpath_id": 15,
            "span_id": i,
            "kind": "geometry_refit",
            "label": format!("GeometryRefit {i} — resampled arc segment"),
            "payload": null,
            "start_move": i * 6,
            "end_move": i * 6 + 6,
            "is_boundary": false,
            "sample_count": 44,
            "total_runtime_s": 0.418_273_2,
            "cutting_runtime_s": 0.401_112_9,
            "rapid_runtime_s": 0.0,
            "air_cut_time_s": 0.017_160_3,
            "air_cut_pct_of_total_runtime": 4.102_5,
            "air_cut_pct_of_cutting_time": 4.278_1,
            "low_engagement_time_s": 0.0,
            "wasted_runtime_s": 0.017_160_3,
            "average_engagement": 0.031_44,
            "per_sample_peak_chipload_mm_per_tooth": 0.041_2,
            "peak_axial_doc_mm": 0.312,
            "peak_plunge_descent_mm": 0.0,
            "total_removed_volume_est_mm3": 12.884,
            "average_mrr_mm3_s": 30.81,
        })
    }

    /// **The exhibit — pre-fix bytes are reproducible, not remembered.**
    ///
    /// B-1's census: 35,838 span-summary rows, 94 % of a 56,225,225-byte
    /// payload. Asking this machinery for the *unbounded* build reproduces a
    /// response in that class; the shipped default reproduces one three
    /// orders of magnitude smaller. Both numbers are asserted here so the
    /// red state stays executable.
    #[test]
    fn unbounded_build_is_enormous_and_the_default_cap_is_not() {
        const POPULATION: usize = 35_838;

        let mut unbounded = ResponseBudget::unbounded();
        let all = cap_json_values(
            POPULATION,
            (0..POPULATION).map(span_summary_row),
            usize::MAX,
            &mut unbounded,
        );
        assert_eq!(all.returned(), POPULATION);
        assert!(!all.truncated());
        let unbounded_bytes = serde_json::to_string(&all.into_value()).unwrap().len();
        assert!(
            unbounded_bytes > 20_000_000,
            "the pre-fix shape must stay reproducible: an uncapped span_summaries \
             array of B-1's measured population serialises to {unbounded_bytes} bytes \
             (B-1 measured 56,225,225 for the whole response, 94 % of it this array)",
        );

        let mut bounded = ResponseBudget::new(MAX_RESPONSE_BYTES);
        let capped = cap_json_values(
            POPULATION,
            (0..POPULATION).map(span_summary_row),
            DEFAULT_MAX_SPAN_SUMMARIES,
            &mut bounded,
        );
        assert_eq!(capped.returned(), DEFAULT_MAX_SPAN_SUMMARIES);
        assert_eq!(capped.total_matching(), POPULATION);
        assert!(capped.truncated());
        let bounded_bytes = serde_json::to_string(&capped.into_value()).unwrap().len();
        assert!(
            bounded_bytes < MAX_RESPONSE_BYTES,
            "bounded build must fit the backstop: {bounded_bytes} B",
        );
        assert!(
            unbounded_bytes / bounded_bytes > 100,
            "the cap must be the difference between two classes of response, not a \
             trim: {unbounded_bytes} B vs {bounded_bytes} B",
        );
    }

    #[test]
    fn cap_keys_round_trip_and_total_matching_is_the_pre_cap_value() {
        let mut budget = ResponseBudget::new(MAX_RESPONSE_BYTES);
        let capped = cap_json_values(1_000, (0..1_000).map(span_summary_row), 7, &mut budget);
        let mut map = Map::new();
        capped.insert_keys("span_summaries", &mut map);

        assert_eq!(map["span_summaries_total_matching"], Value::from(1_000));
        assert_eq!(map["span_summaries_returned"], Value::from(7));
        assert_eq!(map["span_summaries_truncated"], Value::Bool(true));
        assert_eq!(map["span_summaries_cap"], Value::from(7));
        assert!(
            capped.returned() <= capped.cap(),
            "returned must never exceed the cap",
        );
    }

    #[test]
    fn an_uncapped_array_reports_a_null_cap_and_no_truncation() {
        let mut budget = ResponseBudget::new(MAX_RESPONSE_BYTES);
        let capped = cap_json_values(3, (0..3).map(span_summary_row), usize::MAX, &mut budget);
        let mut map = Map::new();
        capped.insert_keys("toolpath_summaries", &mut map);
        assert_eq!(map["toolpath_summaries_cap"], Value::Null);
        assert_eq!(map["toolpath_summaries_truncated"], Value::Bool(false));
        assert_eq!(map["toolpath_summaries_returned"], Value::from(3));
    }

    /// Determinism: the same population capped twice yields the same rows in
    /// the same order. A cap nobody can reproduce is not a contract.
    #[test]
    fn capping_is_deterministic_and_takes_the_leading_entries() {
        let take = |cap: usize| {
            let mut b = ResponseBudget::new(MAX_RESPONSE_BYTES);
            cap_json_values(500, (0..500).map(span_summary_row), cap, &mut b).into_value()
        };
        assert_eq!(take(9), take(9), "same inputs must give the same output");
        let nine = take(9);
        let ids: Vec<i64> = nine
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v["span_id"].as_i64())
            .collect();
        assert_eq!(
            ids,
            (0..9).collect::<Vec<i64>>(),
            "a cap must take the LEADING entries of the documented order, not a \
             sample: ordering contract is {ORDERING_SPAN_SUMMARIES}",
        );
    }

    /// **C25's standing rule.** A section that did not fit is absent and
    /// named — never an empty array, never a zero that reads as a
    /// measurement.
    #[test]
    fn a_section_that_does_not_fit_is_omitted_and_named_never_zeroed() {
        let mut resp = BoundedResponse::new(400);
        resp.insert_always("summary", serde_json::json!({"sample_count": 1_588_883}));
        let big: Vec<Value> = (0..50).map(span_summary_row).collect();
        let included = resp.insert_section("span_summaries", Value::Array(big));
        assert!(!included, "the fixture must overflow, or it proves nothing");

        let out = resp.finish();
        assert_eq!(out["complete"], Value::Bool(false));
        assert_eq!(
            out["sections_not_computed"],
            serde_json::json!(["span_summaries"]),
        );
        assert!(
            out.get("span_summaries").is_none(),
            "a dropped section must be ABSENT; an empty array would be \
             indistinguishable from a real empty result (C25)",
        );
        assert_eq!(
            out["summary"]["sample_count"],
            Value::from(1_588_883),
            "the sections that fit must still be served",
        );
    }

    #[test]
    fn a_response_that_fits_reports_complete_with_an_empty_not_computed_list() {
        let mut resp = BoundedResponse::new(MAX_RESPONSE_BYTES);
        resp.insert_always("summary", serde_json::json!({"sample_count": 42}));
        let mut budget = ResponseBudget::new(MAX_RESPONSE_BYTES);
        let capped = cap_json_values(3, (0..3).map(span_summary_row), 200, &mut budget);
        resp.insert_capped("span_summaries", capped);
        let out = resp.finish();
        assert_eq!(out["complete"], Value::Bool(true));
        assert_eq!(out["sections_not_computed"], serde_json::json!([]));
        assert_eq!(out["span_summaries_truncated"], Value::Bool(false));
        assert_eq!(out["max_response_bytes"], Value::from(MAX_RESPONSE_BYTES));
    }

    /// The budget stops a build even when the item cap would not have. The
    /// array still reports honestly: fewer returned than matching, and
    /// `truncated` is true.
    #[test]
    fn the_byte_budget_can_truncate_before_the_item_cap_does() {
        let mut budget = ResponseBudget::new(2_000);
        let capped = cap_json_values(1_000, (0..1_000).map(span_summary_row), 900, &mut budget);
        assert!(
            capped.returned() < 900,
            "the byte budget must bind first here, or the fixture is wrong \
             (returned {})",
            capped.returned(),
        );
        assert!(capped.truncated());
        assert_eq!(capped.total_matching(), 1_000);
        assert!(budget.used() <= 2_000, "the budget must not be overspent");
    }

    /// The declared backstop is the ruled one. If someone moves this
    /// constant, they move a Checkpoint L ruling and this test says so.
    #[test]
    fn the_ruled_defaults_are_the_shipped_defaults() {
        assert_eq!(MAX_RESPONSE_BYTES, 8 * 1024 * 1024);
        assert_eq!(DEFAULT_MAX_SPAN_SUMMARIES, 200);
        assert_eq!(DEFAULT_MAX_SEMANTIC_SUMMARIES, 200);
        assert_eq!(DEFAULT_MAX_DRILL_SAMPLES, 500);
        assert_eq!(DEFAULT_MAX_TOP_LEVEL_SPANS, 200);
    }
}
