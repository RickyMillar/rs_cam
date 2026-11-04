//! CONTRACT K3 (feeds contract P6): every tool fact states how it is known.
//!
//! A measured tool fact (stick-out, flute length, flute count, shank
//! diameter, taper half-angle, centre cutting) carries its provenance.
//! `ToolFact` has no `Default` and no `Deref`: a reader picks
//! [`ToolFact::known`] or [`ToolFact::planning`] and so states whether an
//! estimate is acceptable.
//!
//! Commit A of P6 adds the type only. `ToolConfig` still stores bare
//! numbers; commit C stores a `ToolFact` behind the `_planning_` readers.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactKind {
    Stated,
    Catalogue,
    Estimated,
    Unknown,
}

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
}

/// Does the tool cut at its centre? `Unknown` (as a [`ToolFact`]) replaces
/// the old "`None` means centre cutting".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CentreCutting {
    Yes,
    /// The tool does not cut at its centre; the payload is the measured
    /// centre core and blade protrusion.
    No(NonCentreCuttingGeometry),
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
