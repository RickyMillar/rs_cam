//! CONTRACT K3 (feeds contract P6): every tool fact states how it is known.
//!
//! A measured tool fact (stick-out, flute length, flute count, shank
//! diameter, taper half-angle, centre cutting, V tip diameter) carries its
//! provenance. `ToolFact` has no `Default` and no `Deref`: a reader picks
//! [`ToolFact::known`] or [`ToolFact::planning`] and so states whether an
//! estimate is acceptable. `ToolConfig` keeps each fact in a private field
//! and gives two readers: `<fact>_fact()` (the fact with its kind) and
//! `<fact>_planning_..()` (the value that planning uses: the fact's value,
//! else the legacy loader default).
//!
//! The file form (project file, tool library, CLI job file) of a fact is a
//! table; an absent key is `Unknown`:
//!
//! ```toml
//! stickout = { stated = 30.0 }
//! cutting_length = { catalogue = 8.0, source = "DMM DSP1f-D2.0-L08" }
//! flute_count = { estimated = 2, basis = "from the name" }
//! centre_cutting = { stated = "no", centre_core_diameter_mm = 8.0, blade_protrusion_mm = 1.0 }
//! ```
//!
//! A bare number (`stickout = 30.0`) refuses to load (ruling Q9).
//!
//! The compiler refuses a read of a fact field from outside its module:
//!
//! ```compile_fail
//! use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
//! let tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
//! let _ = tool.stickout;
//! ```
//!
//! `ToolFact` has no `Default`:
//!
//! ```compile_fail
//! let _: rs_cam_core::compute::tool_config::ToolFact<f64> = Default::default();
//! ```
//!
//! `ToolFact` has no `Deref` to its value:
//!
//! ```compile_fail
//! let fact = rs_cam_core::compute::tool_config::ToolFact::Stated(30.0_f64);
//! let _: f64 = *fact;
//! ```

use serde::ser::SerializeMap as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::tool_config::NonCentreCuttingGeometry;

/// CONTRACT K3. The value is private to its variant; a reader picks
/// [`Self::known`] or [`Self::planning`].
#[derive(Debug, Clone, PartialEq)]
pub enum ToolFact<T> {
    /// The operator stated this value (measured or read).
    Stated(T),
    /// A vendor catalogue gives this value; `source` names the row.
    Catalogue { value: T, source: String },
    /// An estimate; `basis` names how it was made.
    Estimated { value: T, basis: String },
    /// Nobody stated the value.
    Unknown,
}

/// The tag of a [`ToolFact`]: how the fact is known. The lineage (K8) and
/// the rule outcomes (K5) carry it. It is not named `Confidence`:
/// `diagnostics::Confidence` and `tool_load::verdict::Confidence` exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    Stated,
    Catalogue,
    Estimated,
    Unknown,
}

impl FactKind {
    /// The word that the GUI and MCP print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stated => "stated",
            Self::Catalogue => "catalogue",
            Self::Estimated => "estimated",
            Self::Unknown => "unknown",
        }
    }
}

/// The three file forms of a fact, for the refusal of a bare number.
pub const FACT_FORMS: &str = "{ stated = <value> }, { catalogue = <value>, source = \"..\" } \
     or { estimated = <value>, basis = \"..\" }; delete the key when the value is not known";

/// The basis of each fact that [`super::tool_config::ToolConfig::new_default`]
/// writes.
pub const NEW_DEFAULT_BASIS: &str = "new_default";

impl<T> ToolFact<T> {
    /// The value when a person stated it or a catalogue gives it. An
    /// estimate and an unknown fact give `None`.
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Stated(value) | Self::Catalogue { value, .. } => Some(value),
            Self::Estimated { .. } | Self::Unknown => None,
        }
    }

    /// Any value, with its tag. An unknown fact gives `None`.
    pub fn planning(&self) -> Option<(&T, FactKind)> {
        match self {
            Self::Stated(value) => Some((value, FactKind::Stated)),
            Self::Catalogue { value, .. } => Some((value, FactKind::Catalogue)),
            Self::Estimated { value, .. } => Some((value, FactKind::Estimated)),
            Self::Unknown => None,
        }
    }

    /// The tag of this fact.
    pub fn kind(&self) -> FactKind {
        match self {
            Self::Stated(_) => FactKind::Stated,
            Self::Catalogue { .. } => FactKind::Catalogue,
            Self::Estimated { .. } => FactKind::Estimated,
            Self::Unknown => FactKind::Unknown,
        }
    }

    /// `Unknown`: the serde default of a fact key (an absent key).
    #[must_use]
    pub const fn unknown() -> Self {
        Self::Unknown
    }

    /// True for `Unknown`: the save writes no key.
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// The same fact with an estimate's value: `Estimated` with `basis`.
    pub fn estimated(value: T, basis: &str) -> Self {
        Self::Estimated {
            value,
            basis: basis.to_owned(),
        }
    }
}

impl<T: Copy> ToolFact<T> {
    /// The value of the fact, else `legacy` (an `Unknown` fact).
    #[must_use]
    pub fn value_or(&self, legacy: T) -> T {
        self.planning().map_or(legacy, |(value, _)| *value)
    }
}

impl<T: Serialize> Serialize for ToolFact<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Stated(value) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("stated", value)?;
                map.end()
            }
            Self::Catalogue { value, source } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("catalogue", value)?;
                map.serialize_entry("source", source)?;
                map.end()
            }
            Self::Estimated { value, basis } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("estimated", value)?;
                map.serialize_entry("basis", basis)?;
                map.end()
            }
            Self::Unknown => serializer.serialize_none(),
        }
    }
}

/// The file form of a numeric fact.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct FactWire<T> {
    stated: Option<T>,
    catalogue: Option<T>,
    source: Option<String>,
    estimated: Option<T>,
    basis: Option<String>,
}

/// The one rule of a fact table: one of `stated`, `catalogue`,
/// `estimated`; `source` with `catalogue` only, `basis` with `estimated`
/// only.
fn fact_from_parts<T, E: serde::de::Error>(
    stated: Option<T>,
    catalogue: Option<T>,
    source: Option<String>,
    estimated: Option<T>,
    basis: Option<String>,
) -> Result<ToolFact<T>, E> {
    match (stated, catalogue, estimated, source, basis) {
        (Some(value), None, None, None, None) => Ok(ToolFact::Stated(value)),
        (None, Some(value), None, Some(source), None) => Ok(ToolFact::Catalogue { value, source }),
        (None, None, Some(value), None, Some(basis)) => Ok(ToolFact::Estimated { value, basis }),
        _ => Err(E::custom(format!("a tool fact is one of {FACT_FORMS}"))),
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for ToolFact<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Option::<FactWire<T>>::deserialize(deserializer)? {
            None => Ok(Self::Unknown),
            Some(w) => fact_from_parts(w.stated, w.catalogue, w.source, w.estimated, w.basis),
        }
    }
}

/// The file keys of the tool facts (K3). Each holds a table; an absent key
/// is `Unknown`.
pub const FACT_KEYS: [&str; 7] = [
    "cutting_length",
    "taper_half_angle",
    "shank_diameter",
    "stickout",
    "flute_count",
    "centre_cutting",
    "v_tip_diameter",
];

/// Ruling Q9: the path of each fact key whose value is not a table (a bare
/// number or a bare token), in the order of `tools`. Each item is the path
/// of one tool table in the file (for example `tools[2]`) and the table.
/// The project file, the tool library and the CLI job file read this one
/// rule.
pub fn bare_fact_keys<'a>(
    tools: impl IntoIterator<Item = (String, &'a toml::Value)>,
) -> Vec<String> {
    let mut found = Vec::new();
    for (path, tool) in tools {
        let Some(table) = tool.as_table() else {
            continue;
        };
        for key in FACT_KEYS {
            if table.get(key).is_some_and(|v| !v.is_table()) {
                found.push(format!("{path}.{key}"));
            }
        }
    }
    found
}

/// The one refusal text of [`bare_fact_keys`]: it names every bare key and
/// the three forms.
#[must_use]
pub fn bare_facts_text(paths: &[String]) -> String {
    format!(
        "a tool fact states how it is known (K3); a bare value refuses. Bare: {}. Write each \
         as {FACT_FORMS}",
        paths.join(", ")
    )
}

/// Does the tool cut at its centre? `Unknown` (as a [`ToolFact`]) replaces
/// the old "`None` = a centre-cutting tool".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CentreCutting {
    Yes,
    /// The tool does not cut at its centre; the payload is the measured
    /// centre core and blade protrusion.
    No(NonCentreCuttingGeometry),
}

/// The token of a centre-cutting fact in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum YesNo {
    Yes,
    No,
}

/// The file form of the centre-cutting fact: the token, and the geometry
/// with `"no"`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CentreWire {
    stated: Option<YesNo>,
    catalogue: Option<YesNo>,
    source: Option<String>,
    estimated: Option<YesNo>,
    basis: Option<String>,
    centre_core_diameter_mm: Option<f64>,
    blade_protrusion_mm: Option<f64>,
}

/// The serde form of `ToolFact<CentreCutting>` (`#[serde(with = ..)]`):
/// `{ stated = "yes" }` or `{ stated = "no", centre_core_diameter_mm = ..,
/// blade_protrusion_mm = .. }`, with `catalogue` + `source` or `estimated`
/// + `basis` in place of `stated`.
pub mod centre_cutting_serde {
    use super::{
        CentreCutting, CentreWire, Deserialize as _, Deserializer, NonCentreCuttingGeometry,
        Serializer, ToolFact, YesNo, fact_from_parts,
    };
    use serde::de::Error as _;
    use serde::ser::SerializeMap as _;

    /// Write the fact.
    ///
    /// # Errors
    ///
    /// The serializer's error.
    pub fn serialize<S: Serializer>(
        fact: &ToolFact<CentreCutting>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let (key, value, extra) = match fact {
            ToolFact::Stated(v) => ("stated", v, None),
            ToolFact::Catalogue { value, source } => ("catalogue", value, Some(("source", source))),
            ToolFact::Estimated { value, basis } => ("estimated", value, Some(("basis", basis))),
            ToolFact::Unknown => return serializer.serialize_none(),
        };
        let mut map = serializer.serialize_map(None)?;
        match value {
            CentreCutting::Yes => map.serialize_entry(key, &YesNo::Yes)?,
            CentreCutting::No(_) => map.serialize_entry(key, &YesNo::No)?,
        }
        if let Some((name, text)) = extra {
            map.serialize_entry(name, text)?;
        }
        match value {
            CentreCutting::Yes => {}
            CentreCutting::No(g) => {
                map.serialize_entry("centre_core_diameter_mm", &g.centre_core_diameter_mm)?;
                map.serialize_entry("blade_protrusion_mm", &g.blade_protrusion_mm)?;
            }
        }
        map.end()
    }

    /// Read the fact; an absent key (serde default) or `null` is `Unknown`.
    ///
    /// # Errors
    ///
    /// A table that is not one of the forms.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ToolFact<CentreCutting>, D::Error> {
        let Some(w) = Option::<CentreWire>::deserialize(deserializer)? else {
            return Ok(ToolFact::Unknown);
        };
        let token = fact_from_parts::<YesNo, D::Error>(
            w.stated,
            w.catalogue,
            w.source,
            w.estimated,
            w.basis,
        )?;
        let value = |t: YesNo| -> Result<CentreCutting, D::Error> {
            match (t, w.centre_core_diameter_mm, w.blade_protrusion_mm) {
                (YesNo::Yes, None, None) => Ok(CentreCutting::Yes),
                (YesNo::No, Some(centre_core_diameter_mm), Some(blade_protrusion_mm)) => {
                    Ok(CentreCutting::No(NonCentreCuttingGeometry {
                        centre_core_diameter_mm,
                        blade_protrusion_mm,
                    }))
                }
                (YesNo::Yes, _, _) => Err(D::Error::custom(
                    "centre_cutting \"yes\" takes no centre_core_diameter_mm or \
                     blade_protrusion_mm",
                )),
                (YesNo::No, _, _) => Err(D::Error::custom(
                    "centre_cutting \"no\" needs centre_core_diameter_mm and blade_protrusion_mm \
                     (the measured centre core and blade protrusion, mm)",
                )),
            }
        };
        Ok(match token {
            ToolFact::Stated(t) => ToolFact::Stated(value(t)?),
            ToolFact::Catalogue { value: t, source } => ToolFact::Catalogue {
                value: value(t)?,
                source,
            },
            ToolFact::Estimated { value: t, basis } => ToolFact::Estimated {
                value: value(t)?,
                basis,
            },
            ToolFact::Unknown => ToolFact::Unknown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_gives_its_value_to_the_reader_it_allows() {
        let stated = ToolFact::Stated(30.0_f64);
        let catalogue = ToolFact::Catalogue {
            value: 8.0_f64,
            source: "a catalogue row".to_owned(),
        };
        let estimated = ToolFact::Estimated {
            value: 2_u32,
            basis: "from the name".to_owned(),
        };
        let unknown: ToolFact<f64> = ToolFact::Unknown;

        assert_eq!(stated.known(), Some(&30.0));
        assert_eq!(stated.planning(), Some((&30.0, FactKind::Stated)));
        assert_eq!(catalogue.known(), Some(&8.0));
        assert_eq!(catalogue.planning(), Some((&8.0, FactKind::Catalogue)));
        assert_eq!(estimated.known(), None);
        assert_eq!(estimated.planning(), Some((&2, FactKind::Estimated)));
        assert_eq!(unknown.known(), None);
        assert_eq!(unknown.planning(), None);

        assert_eq!(stated.kind(), FactKind::Stated);
        assert_eq!(catalogue.kind(), FactKind::Catalogue);
        assert_eq!(estimated.kind(), FactKind::Estimated);
        assert_eq!(unknown.kind(), FactKind::Unknown);
    }

    #[test]
    fn a_non_centre_cutting_fact_holds_its_geometry() {
        let geometry = NonCentreCuttingGeometry {
            centre_core_diameter_mm: 8.0,
            blade_protrusion_mm: 1.0,
        };
        let fact = ToolFact::Stated(CentreCutting::No(geometry));
        assert_eq!(fact.known(), Some(&CentreCutting::No(geometry)));
        let yes = ToolFact::Stated(CentreCutting::Yes);
        assert_eq!(yes.kind(), FactKind::Stated);
    }
}
