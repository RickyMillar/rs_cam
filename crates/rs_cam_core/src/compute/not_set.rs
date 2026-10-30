//! S11 (operator ruling 2026-10-23): a value with no derivation is not set.
//!
//! "If there is no logical reason for a default, then dont have it." A
//! drawing has no depth, and a tool with no feeds data has no feed. So a
//! new op holds these values as `None` (the floor as
//! [`crate::compute::floor::Floor::Unset`]). The op stays in the project,
//! and generation refuses with a [`NotSet`] that names each field and the
//! fix. The GUI shows the field empty with a hint, and MCP reports it
//! unset.
//!
//! - Depth ([`NotSetKind::Depth`]): Trace `depth`, V-Carve `max_depth`,
//!   Inlay and Inlay Plug `pocket_depth`, Project Curve `depth`. The floor
//!   of a floor op refuses through the heights door
//!   (`FloorRefusal::Unset`).
//! - Feeds ([`NotSetKind::Feeds`]): `feed_rate`, `plunge_rate` and
//!   `spindle_rpm` of every op. Suggest writes them when the tool has
//!   feeds data; with no data the operator sets them.
//!
//! [`crate::compute::catalog::OperationConfig::not_set`] lists them, and
//! [`crate::compute::catalog::OperationConfig::cut_feeds`] gives the set
//! feeds to the generators.

/// The refusal text of an op with no feeds (S11). The GUI, MCP and the CLI
/// show this one text.
pub const FEEDS_NOT_SET_TEXT: &str = "no feeds data for this tool: set feed and RPM";

/// The hint the GUI shows in an empty field (S11).
pub const SET_A_VALUE_HINT: &str = "set a value";

/// Every parameter name that a [`NotSet`] can name: the depths of
/// `OperationConfig::unset_depths` and the feeds of
/// `OperationConfig::unset_feeds`. A finding of a value that is not set
/// anchors to one of them (`diagnostics::presentation::anchor_field_names`).
/// The unit test holds it against every op.
pub const NOT_SET_FIELDS: &[&str] = &[
    "depth",
    "max_depth",
    "pocket_depth",
    "feed_rate",
    "plunge_rate",
    "spindle_rpm",
];

/// What kind of value is not set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotSetKind {
    /// A depth that no rule gives (a drawing has no depth).
    Depth,
    /// The feeds: Suggest had no data for the tool.
    Feeds,
}

/// An op refuses to generate: values with no derivation are not set (S11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotSet {
    /// The op label (`OperationType::label`).
    pub op: &'static str,
    /// What kind of value is not set.
    pub kind: NotSetKind,
    /// The parameter names that are not set, in schema order. Never empty.
    pub fields: Vec<&'static str>,
}

impl NotSet {
    /// The first field that is not set: the `field` of the session error.
    #[must_use]
    pub fn first_field(&self) -> Option<&'static str> {
        self.fields.first().copied()
    }

    /// The operator text of the refusal, for the diagnostic detail: plain
    /// field names, no parameter keys. The `Display` text (the generation
    /// refusal) keeps the keys for an agent; the diagnostic anchor holds
    /// the first key.
    #[must_use]
    pub fn detail(&self) -> String {
        let names: Vec<&str> = self.fields.iter().map(|name| field_phrase(name)).collect();
        let list = match names.as_slice() {
            [] => String::new(),
            [one] => (*one).to_owned(),
            [head @ .., last] => format!("{} and {last}", head.join(", ")),
        };
        match self.kind {
            NotSetKind::Depth => format!(
                "{}: no rule gives a depth for this operation. Set the {list}.",
                self.op
            ),
            NotSetKind::Feeds => format!(
                "{}: no feed data for this tool. Set the {list} on the Feeds tab.",
                self.op
            ),
        }
    }
}

/// The operator name of a [`NOT_SET_FIELDS`] key ("feed_rate" gives "feed
/// rate"). A key with no entry reads as itself.
#[must_use]
pub fn field_phrase(name: &str) -> &str {
    match name {
        "depth" => "depth",
        "max_depth" => "maximum depth",
        "pocket_depth" => "pocket depth",
        "feed_rate" => "feed rate",
        "plunge_rate" => "plunge rate",
        "spindle_rpm" => "spindle speed",
        other => other,
    }
}

impl std::fmt::Display for NotSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fields = self
            .fields
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        match self.kind {
            NotSetKind::Depth => write!(
                f,
                "{}: the depth is not set: no rule gives a depth for this op. Set {fields}",
                self.op
            ),
            NotSetKind::Feeds => write!(f, "{}: {FEEDS_NOT_SET_TEXT} (not set: {fields})", self.op),
        }
    }
}

/// The feeds of an op whose feeds are all set ([`NotSet`] otherwise). The
/// generators read these numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutFeeds {
    /// The cut feed (mm/min).
    pub feed_rate: f64,
    /// The plunge feed (mm/min).
    pub plunge_rate: f64,
    /// The spindle speed (RPM).
    pub spindle_rpm: u32,
}

/// The depth `value` of the field `field` of the op labelled `op`, or the
/// [`NotSet`] refusal when it is not set. The prologue refuses first; a
/// generator reads its depth through this door, so it never cuts an unset
/// depth.
///
/// # Errors
///
/// A [`NotSet`] of kind `Depth` that names `field`.
pub fn required_depth(
    value: Option<f64>,
    op: &'static str,
    field: &'static str,
) -> Result<f64, NotSet> {
    value.ok_or_else(|| NotSet {
        op,
        kind: NotSetKind::Depth,
        fields: vec![field],
    })
}

#[cfg(test)]
mod tests {
    use super::NOT_SET_FIELDS;
    use crate::compute::catalog::{OperationConfig, OperationType};

    /// W3f: each field that an op can name as not set is in
    /// [`NOT_SET_FIELDS`]. Each op is checked with no feeds and with its
    /// default depths (S11: a depth that no rule gives is not set).
    #[test]
    fn not_set_fields_name_every_unset_field_w3f() {
        let mut named = 0_usize;
        for op in OperationType::ALL {
            let mut config = OperationConfig::new_default(*op);
            let params = config.as_params_mut();
            params.unset_feed_rate();
            params.unset_plunge_rate();
            params.set_spindle_rpm(None);
            for field in config
                .unset_depths()
                .into_iter()
                .chain(config.unset_feeds())
            {
                named += 1;
                assert!(
                    NOT_SET_FIELDS.contains(&field),
                    "{op:?} names `{field}`, which NOT_SET_FIELDS does not hold"
                );
            }
        }
        assert!(named > OperationType::ALL.len(), "non-vacuity: {named}");
    }
}
