//! The domain check of an operation config at every door (W6b: F030,
//! F036) and the one refusal every entity domain gives (F210).
//!
//! # The operation check
//!
//! [`validate_operation`] is the one check of a whole
//! [`OperationConfig`]. It holds three rules:
//!
//! 1. Each published number is finite and inside the range its
//!    `#[param]` declares ([`check_param`], the rule `set_toolpath_param`
//!    reads for one value). An absent `Option` holds no number.
//! 2. An ordered pair of angles is in order ([`ordered_pair`], F036): a
//!    slope band with `slope_from > slope_to`, or a Unified Finish with
//!    `steep_threshold_deg > waterline_threshold_deg`, admits no cell.
//! 3. The driver-set `floor` of the four floor ops passes
//!    `Floor::door_refusal` (selection P3.2), because the derive check of
//!    W6a does not reach inside it.
//! 4. A Profile with side `On` does not compensate `InControl`
//!    ([`side_on_in_control`], P6.1, L-5): a cut on the line
//!    has no side for G41/G42.
//!
//! The session calls it at `AddToolpath`, at the op change
//! (`SetToolpathOperation`), in the generic arm of `set_toolpath_param`
//! and at project load. `tests/an_op_outside_its_domain_is_refused_at_every_door_w6b.rs`
//! drives each door.
//!
//! # The value read
//!
//! The check reads each published number by its name from the TOML form
//! of the config, the form a project file holds. The JSON form is not
//! used: `serde_json` writes a `NaN` as `null`, so a non-finite number
//! would read as an absent `Option` and pass.

use super::catalog::{OperationConfig, ParamDef, ParamRange};
use super::floor::FloorRefusal;

/// Why an operation config is outside its domain.
#[derive(Debug, Clone, PartialEq)]
pub enum OpDomainRefusal {
    /// A published number is outside its declared range, or not finite.
    OutOfRange {
        /// The op label.
        op: &'static str,
        /// The parameter name.
        param: String,
        /// The value.
        value: f64,
        /// The domain, as [`ParamRange::describe`] states it.
        domain: String,
    },
    /// The low end of an ordered pair is above the high end (F036). The
    /// band admits nothing.
    InvertedPair {
        /// The op label.
        op: &'static str,
        /// The name of the low end.
        low: &'static str,
        /// Its value.
        low_value: f64,
        /// The name of the high end.
        high: &'static str,
        /// Its value.
        high_value: f64,
    },
    /// The floor of the op is refused at the door (P3.2).
    Floor {
        /// The op label.
        op: &'static str,
        /// The refusal.
        refusal: FloorRefusal,
    },
    /// A Profile with side `On` asks for `InControl` compensation (P6.1,
    /// L-5). A cut on the line has no side for G41/G42.
    SideOnInControl {
        /// The op label.
        op: &'static str,
    },
    /// The config has no TOML form, so its numbers cannot be read.
    Unreadable {
        /// The op label.
        op: &'static str,
        /// The serializer text.
        detail: String,
    },
}

impl std::fmt::Display for OpDomainRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange {
                op,
                param,
                value,
                domain,
            } => write!(
                f,
                "'{param}' = {value} is outside the accepted range for {op} ({domain})"
            ),
            Self::InvertedPair {
                op,
                low,
                low_value,
                high,
                high_value,
            } => write!(
                f,
                "{op} '{low}' = {low_value} is above '{high}' = {high_value}; the band from \
                 '{low}' to '{high}' then admits nothing. Set '{low}' at or below '{high}'"
            ),
            Self::Floor { op, refusal } => write!(f, "{op} floor: {refusal}"),
            Self::SideOnInControl { op } => write!(
                f,
                "{op} side On cuts on the line; it has no side for G41/G42. Set compensation \
                 to In Computer"
            ),
            Self::Unreadable { op, detail } => {
                write!(
                    f,
                    "the {op} config cannot be read for its domain check: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for OpDomainRefusal {}

/// The one range rule of one published number of `operation` (DR-LIVE,
/// F185). `set_toolpath_param` calls it for the value it is about to
/// write; [`validate_operation`] calls it for each stored number.
///
/// A non-finite value is always refused. A name with no declared range
/// accepts every finite value: a published number without a range is an
/// absolute Z (`frame_dependent`) or states `unbounded = "<reason>"`.
///
/// # Errors
///
/// [`OpDomainRefusal::OutOfRange`].
pub fn check_param(
    operation: &OperationConfig,
    param: &str,
    value: f64,
) -> Result<(), OpDomainRefusal> {
    let range = operation.param_range(param);
    let accepted = match range {
        Some(range) => range.accepts(value),
        None => value.is_finite(),
    };
    if accepted {
        return Ok(());
    }
    Err(OpDomainRefusal::OutOfRange {
        op: operation.label(),
        param: param.to_owned(),
        value,
        domain: range.map_or_else(|| "finite".to_owned(), |r| ParamRange::describe(&r)),
    })
}

/// The ordered pair of angles of `operation`, as `(low name, low, high
/// name, high)`, or `None` for an op with no ordered pair (F036). The
/// match names every variant and reads the fields by type, so a new op
/// states whether it holds a pair, and a renamed field does not compile.
#[must_use]
pub fn ordered_pair(operation: &OperationConfig) -> Option<(&'static str, f64, &'static str, f64)> {
    match operation {
        OperationConfig::DropCutter(c) => {
            Some(("slope_from", c.slope_from, "slope_to", c.slope_to))
        }
        OperationConfig::Scallop(c) => Some(("slope_from", c.slope_from, "slope_to", c.slope_to)),
        OperationConfig::RampFinish(c) => {
            Some(("slope_from", c.slope_from, "slope_to", c.slope_to))
        }
        OperationConfig::UnifiedFinish(c) => Some((
            "steep_threshold_deg",
            c.steep_threshold_deg,
            "waterline_threshold_deg",
            c.waterline_threshold_deg,
        )),
        OperationConfig::Pocket(_)
        | OperationConfig::Profile(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Face(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Drill(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_)
        | OperationConfig::ProjectCurve(_)
        | OperationConfig::AlignmentPinDrill(_) => None,
    }
}

/// Whether `operation` is a Profile with side `On` and `InControl`
/// compensation (P6.1, L-5). The controller offsets the tool to one side
/// of the path (G41/G42), and a cut on the line has no side, so the pair
/// is refused at every door.
#[must_use]
pub fn side_on_in_control(operation: &OperationConfig) -> bool {
    use super::operation_configs::{CompensationType, ProfileSide};
    matches!(
        operation,
        OperationConfig::Profile(c)
            if c.side == ProfileSide::On && c.compensation == CompensationType::InControl
    )
}

/// The number a TOML value holds, or `None` for a value that is not a
/// number. An integer reads as its `f64`.
// SAFETY: a published integer is a count or a size far below 2^53, and the
// check only compares it with a declared bound.
#[allow(clippy::cast_precision_loss)]
fn toml_number(value: &toml::Value) -> Option<f64> {
    match value {
        toml::Value::Float(f) => Some(*f),
        toml::Value::Integer(i) => Some(*i as f64),
        toml::Value::String(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_)
        | toml::Value::Array(_)
        | toml::Value::Table(_) => None,
    }
}

/// The published numbers of `operation` with their values, in def order.
/// An absent `Option` is not in the list.
fn published_numbers(
    operation: &OperationConfig,
) -> Result<Vec<(&'static ParamDef, f64)>, OpDomainRefusal> {
    let unreadable = |detail: String| OpDomainRefusal::Unreadable {
        op: operation.label(),
        detail,
    };
    let value = toml::Value::try_from(operation).map_err(|e| unreadable(e.to_string()))?;
    let params = value
        .get("params")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| unreadable("no params table".to_owned()))?;
    Ok(operation
        .op_type()
        .registry_entry()
        .params
        .defs
        .iter()
        .filter(|def| def.kind.is_number())
        .filter_map(|def| {
            params
                .get(def.name)
                .and_then(toml_number)
                .map(|value| (def, value))
        })
        .collect())
}

/// Check every rule of the module doc on `operation`.
///
/// # Errors
///
/// The first [`OpDomainRefusal`] in def order: a number out of range, then
/// an inverted pair, then the floor, then side `On` with `InControl`.
pub fn validate_operation(operation: &OperationConfig) -> Result<(), OpDomainRefusal> {
    for (def, value) in published_numbers(operation)? {
        check_param(operation, def.name, value)?;
    }
    if let Some((low, low_value, high, high_value)) = ordered_pair(operation)
        && low_value > high_value
    {
        return Err(OpDomainRefusal::InvertedPair {
            op: operation.label(),
            low,
            low_value,
            high,
            high_value,
        });
    }
    if let Some(refusal) = operation.floor().and_then(|floor| floor.door_refusal()) {
        return Err(OpDomainRefusal::Floor {
            op: operation.label(),
            refusal,
        });
    }
    if side_on_in_control(operation) {
        return Err(OpDomainRefusal::SideOnInControl {
            op: operation.label(),
        });
    }
    Ok(())
}

/// The refusal of a value outside the domain of a project entity: an
/// operation config (W6b), the stock or the machine (F210). The session
/// door wraps it in `SessionError::OutOfDomain`.
#[derive(Debug, Clone, PartialEq)]
pub enum DomainRefusal {
    /// The operation config of the toolpath `toolpath`.
    Operation {
        /// The toolpath name.
        toolpath: String,
        /// The refusal.
        refusal: OpDomainRefusal,
    },
    /// The stock.
    Stock(super::stock_domain::StockDomainError),
    /// The machine profile.
    Machine(crate::machine::domain::MachineDomainError),
    /// The machine head.
    Head(crate::machine::head::HeadRefusal),
    /// The shoe setting of the toolpath `toolpath`.
    Shoe {
        /// The toolpath name.
        toolpath: String,
        /// The refusal.
        refusal: crate::machine::head::HeadRefusal,
    },
}

impl std::fmt::Display for DomainRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Operation { toolpath, refusal } => {
                write!(f, "Toolpath '{toolpath}' refused: {refusal}")
            }
            Self::Stock(error) => write!(f, "Stock refused: {error}"),
            Self::Machine(error) => write!(f, "Machine refused: {error}"),
            Self::Head(error) => write!(f, "Machine head refused: {error}"),
            Self::Shoe { toolpath, refusal } => {
                write!(f, "Toolpath '{toolpath}' shoe refused: {refusal}")
            }
        }
    }
}

impl std::error::Error for DomainRefusal {}

impl From<DomainRefusal> for crate::session::SessionError {
    fn from(refusal: DomainRefusal) -> Self {
        Self::OutOfDomain(refusal)
    }
}

/// [`validate_operation`] on `operation`, with the toolpath name in the
/// refusal.
///
/// # Errors
///
/// [`DomainRefusal::Operation`].
pub fn validate_toolpath_operation(
    toolpath: &str,
    operation: &OperationConfig,
) -> Result<(), DomainRefusal> {
    validate_operation(operation).map_err(|refusal| DomainRefusal::Operation {
        toolpath: toolpath.to_owned(),
        refusal,
    })
}

/// The domain check of a whole toolpath config: its operation
/// ([`validate_operation`]) and its shoe setting
/// ([`crate::machine::head::ShoeSetting::validate`]). `AddToolpath` and the
/// project loader call it.
///
/// # Errors
///
/// [`DomainRefusal::Operation`] or [`DomainRefusal::Shoe`].
pub fn validate_toolpath(config: &crate::session::ToolpathConfig) -> Result<(), DomainRefusal> {
    validate_toolpath_operation(&config.name, &config.operation)?;
    config
        .shoe
        .validate()
        .map_err(|refusal| DomainRefusal::Shoe {
            toolpath: config.name.clone(),
            refusal,
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;

    #[test]
    fn every_default_is_inside_its_domain() {
        for op in OperationType::ALL {
            let config = OperationConfig::new_default(*op);
            if let Err(refusal) = validate_operation(&config) {
                panic!("{op:?} default refused: {refusal}");
            }
        }
    }

    #[test]
    fn a_nan_is_read_as_a_number_not_as_absent() {
        let mut config = OperationConfig::new_default(OperationType::Pocket);
        config.set_feed_rate(f64::NAN);
        let refusal = validate_operation(&config).unwrap_err();
        assert!(
            matches!(&refusal, OpDomainRefusal::OutOfRange { param, .. } if param == "feed_rate"),
            "{refusal}"
        );
    }

    #[test]
    fn every_pair_names_two_published_defs() {
        for op in OperationType::ALL {
            let config = OperationConfig::new_default(*op);
            let Some((low, _, high, _)) = ordered_pair(&config) else {
                continue;
            };
            let names = OperationConfig::param_names_for_type(*op);
            assert!(names.contains(&low), "{op:?} does not publish {low}");
            assert!(names.contains(&high), "{op:?} does not publish {high}");
        }
    }
}
